use go_model::*;
use thiserror::Error;
use treesitter_types_go::{self as cst, FromNode, LeafNode, Spanned};

#[derive(Debug, Error)]
pub enum WalkError {
    #[error("unexpected node: {kind}")]
    UnexpectedNode { kind: String },
    #[error("missing required field: {field}")]
    MissingField { field: String },
    #[error("tree-sitter parse error")]
    ParseError(#[from] cst::ParseError),
}

type R<T> = Result<T, WalkError>;

fn cvt_span(s: &cst::Span) -> Span {
    Span {
        start_byte: s.start_byte,
        end_byte: s.end_byte,
        start_row: s.start.row,
        start_col: s.start.column,
        end_row: s.end.row,
        end_col: s.end.column,
    }
}

fn sp(s: &impl Spanned) -> Span {
    cvt_span(&s.span())
}

/// Extract text from source bytes using a span.
fn text_from_span<'a>(src: &'a [u8], span: &cst::Span) -> &'a str {
    std::str::from_utf8(&src[span.start_byte..span.end_byte]).unwrap_or("")
}

// --- Source File ---

pub fn walk_source_file(cst_file: &cst::SourceFile<'_>, src: &[u8]) -> R<SourceFile> {
    let mut package = None;
    let mut imports = Vec::new();
    let mut decls = Vec::new();

    for child in &cst_file.children {
        match child {
            cst::SourceFileChildren::PackageClause(pkg) => {
                package = Some(Ident {
                    name: pkg.children.text().to_owned(),
                    span: sp(&pkg.children),
                });
            }
            cst::SourceFileChildren::ImportDeclaration(imp) => {
                walk_import_decl(imp, &mut imports, src)?;
            }
            cst::SourceFileChildren::FunctionDeclaration(f) => {
                decls.push(TopLevelDecl::Func(Box::new(walk_func_decl(f, src)?)));
            }
            cst::SourceFileChildren::MethodDeclaration(m) => {
                decls.push(TopLevelDecl::Method(Box::new(walk_method_decl(m, src)?)));
            }
            cst::SourceFileChildren::Statement(stmt) => {
                walk_top_level_stmt(stmt, &mut decls, &mut imports, src)?;
            }
        }
    }

    let package = package.ok_or_else(|| WalkError::MissingField {
        field: "package".into(),
    })?;

    Ok(SourceFile {
        package,
        imports,
        decls,
        span: sp(cst_file),
    })
}

fn walk_top_level_stmt(
    stmt: &cst::Statement<'_>,
    decls: &mut Vec<TopLevelDecl>,
    _imports: &mut Vec<ImportSpec>,
    src: &[u8],
) -> R<()> {
    match stmt {
        cst::Statement::TypeDeclaration(td) => {
            decls.push(TopLevelDecl::Type(walk_type_decl(td, src)?));
        }
        cst::Statement::VarDeclaration(vd) => {
            decls.push(TopLevelDecl::Var(walk_var_decl(vd, src)?));
        }
        cst::Statement::ConstDeclaration(cd) => {
            decls.push(TopLevelDecl::Const(walk_const_decl(cd, src)?));
        }
        // SimpleStatement at top level wraps expression statements etc.
        cst::Statement::SimpleStatement(ss) => {
            // Simple statements at top level are unusual; skip silently
            let _ = ss;
        }
        _ => {}
    }
    Ok(())
}

// --- Imports ---

fn walk_import_decl(
    imp: &cst::ImportDeclaration<'_>,
    out: &mut Vec<ImportSpec>,
    src: &[u8],
) -> R<()> {
    match &imp.children {
        cst::ImportDeclarationChildren::ImportSpec(spec) => {
            out.push(walk_import_spec(spec, src)?);
        }
        cst::ImportDeclarationChildren::ImportSpecList(list) => {
            for spec in &list.children {
                out.push(walk_import_spec(spec, src)?);
            }
        }
    }
    Ok(())
}

fn walk_import_spec(spec: &cst::ImportSpec<'_>, src: &[u8]) -> R<ImportSpec> {
    let alias = match &spec.name {
        None => ImportAlias::Implicit,
        Some(cst::ImportSpecName::Dot(_)) => ImportAlias::Dot,
        Some(cst::ImportSpecName::BlankIdentifier(_)) => ImportAlias::Blank,
        Some(cst::ImportSpecName::PackageIdentifier(id)) => ImportAlias::Named(Ident {
            name: id.text().to_owned(),
            span: sp(id.as_ref()),
        }),
    };

    let path = match &spec.path {
        cst::ImportSpecPath::InterpretedStringLiteral(s) => StringLit {
            raw: text_from_span(src, &s.span).to_owned(),
            span: sp(s.as_ref()),
        },
        cst::ImportSpecPath::RawStringLiteral(s) => StringLit {
            raw: text_from_span(src, &s.span).to_owned(),
            span: sp(s.as_ref()),
        },
    };

    Ok(ImportSpec {
        alias,
        path,
        span: sp(spec),
    })
}

// --- Type Declarations ---

fn walk_type_decl(td: &cst::TypeDeclaration<'_>, src: &[u8]) -> R<Vec<TypeSpec>> {
    td.children
        .iter()
        .map(|child| match child {
            cst::TypeDeclarationChildren::TypeSpec(ts) => walk_type_spec(ts, src),
            cst::TypeDeclarationChildren::TypeAlias(ta) => walk_type_alias(ta, src),
        })
        .collect()
}

fn walk_type_spec(ts: &cst::TypeSpec<'_>, src: &[u8]) -> R<TypeSpec> {
    let name = Ident {
        name: ts.name.text().to_owned(),
        span: sp(&ts.name),
    };
    let type_params = walk_opt_type_param_list(&ts.type_parameters, src)?;
    let ty = walk_type(&ts.r#type, src)?;
    Ok(TypeSpec::Def {
        name,
        type_params,
        ty,
        span: sp(ts),
    })
}

fn walk_type_alias(ta: &cst::TypeAlias<'_>, src: &[u8]) -> R<TypeSpec> {
    let name = Ident {
        name: ta.name.text().to_owned(),
        span: sp(&ta.name),
    };
    let type_params = walk_opt_type_param_list(&ta.type_parameters, src)?;
    let ty = walk_type(&ta.r#type, src)?;
    Ok(TypeSpec::Alias {
        name,
        type_params,
        ty,
        span: sp(ta),
    })
}

// --- Var/Const Declarations ---

fn walk_var_decl(vd: &cst::VarDeclaration<'_>, src: &[u8]) -> R<Vec<VarSpec>> {
    match &vd.children {
        cst::VarDeclarationChildren::VarSpec(vs) => Ok(vec![walk_var_spec(vs, src)?]),
        cst::VarDeclarationChildren::VarSpecList(list) => list
            .children
            .iter()
            .map(|vs| walk_var_spec(vs, src))
            .collect(),
    }
}

fn walk_var_spec(vs: &cst::VarSpec<'_>, src: &[u8]) -> R<VarSpec> {
    let names = vs.name.iter().map(|n| ident_from_id(n)).collect();
    let ty = vs.r#type.as_ref().map(|t| walk_type(t, src)).transpose()?;
    let values = vs
        .value
        .as_ref()
        .map(|el| walk_expr_list(el, src))
        .transpose()?
        .unwrap_or_default();
    Ok(VarSpec {
        names,
        ty,
        values,
        span: sp(vs),
    })
}

fn walk_const_decl(cd: &cst::ConstDeclaration<'_>, src: &[u8]) -> R<Vec<ConstSpec>> {
    cd.children
        .iter()
        .map(|cs| walk_const_spec(cs, src))
        .collect()
}

fn walk_const_spec(cs: &cst::ConstSpec<'_>, src: &[u8]) -> R<ConstSpec> {
    let names = cs
        .name
        .iter()
        .filter_map(|n| match n {
            cst::ConstSpecName::Identifier(id) => Some(ident_from_id(id)),
            cst::ConstSpecName::Comma(_) => None,
        })
        .collect();
    let ty = cs.r#type.as_ref().map(|t| walk_type(t, src)).transpose()?;
    let values = cs
        .value
        .as_ref()
        .map(|el| walk_expr_list(el, src))
        .transpose()?
        .unwrap_or_default();
    Ok(ConstSpec {
        names,
        ty,
        values,
        span: sp(cs),
    })
}

// --- Function/Method Declarations ---

fn walk_func_decl(f: &cst::FunctionDeclaration<'_>, src: &[u8]) -> R<FuncDecl> {
    let name = ident_from_id(&f.name);
    let type_params = walk_opt_type_param_list(&f.type_parameters, src)?;
    let (params, _) = walk_param_list(&f.parameters, src)?;
    let results = walk_opt_result_func_decl(&f.result, src)?;
    let body = f.body.as_ref().map(|b| walk_block(b, src)).transpose()?;
    Ok(FuncDecl {
        name,
        ty: FuncType {
            type_params,
            params,
            results,
            span: sp(f),
        },
        body,
        doc: None,
        span: sp(f),
    })
}

fn walk_method_decl(m: &cst::MethodDeclaration<'_>, src: &[u8]) -> R<MethodDecl> {
    let name = Ident {
        name: m.name.text().to_owned(),
        span: sp(&m.name),
    };
    let receiver = walk_receiver(&m.receiver, src)?;
    let (params, _) = walk_param_list(&m.parameters, src)?;
    let results = walk_opt_result_method_decl(&m.result, src)?;
    let body = m.body.as_ref().map(|b| walk_block(b, src)).transpose()?;
    Ok(MethodDecl {
        receiver,
        name,
        ty: FuncType {
            type_params: vec![],
            params,
            results,
            span: sp(m),
        },
        body,
        doc: None,
        span: sp(m),
    })
}

fn walk_receiver(pl: &cst::ParameterList<'_>, src: &[u8]) -> R<Receiver> {
    if let Some(child) = pl.children.first() {
        match child {
            cst::ParameterListChildren::ParameterDeclaration(pd) => {
                let name = pd.name.first().map(|n| ident_from_id(n));
                let ty = walk_type(&pd.r#type, src)?;
                return Ok(Receiver {
                    name,
                    type_params: vec![],
                    ty,
                    span: sp(pl),
                });
            }
            cst::ParameterListChildren::VariadicParameterDeclaration(_) => {
                return Err(WalkError::UnexpectedNode {
                    kind: "variadic receiver".into(),
                });
            }
        }
    }
    Err(WalkError::MissingField {
        field: "receiver parameter".into(),
    })
}

fn walk_param_list(pl: &cst::ParameterList<'_>, src: &[u8]) -> R<(Vec<ParamDecl>, bool)> {
    let mut params = Vec::new();
    let mut has_variadic = false;
    for child in &pl.children {
        match child {
            cst::ParameterListChildren::ParameterDeclaration(pd) => {
                let names: Vec<Ident> = pd.name.iter().map(|n| ident_from_id(n)).collect();
                let ty = walk_type(&pd.r#type, src)?;
                params.push(ParamDecl {
                    names,
                    ty,
                    variadic: false,
                    span: sp(pd.as_ref()),
                });
            }
            cst::ParameterListChildren::VariadicParameterDeclaration(vpd) => {
                has_variadic = true;
                let names: Vec<Ident> = vpd.name.iter().map(|n| ident_from_id(n)).collect();
                let ty = walk_type(&vpd.r#type, src)?;
                params.push(ParamDecl {
                    names,
                    ty,
                    variadic: true,
                    span: sp(vpd.as_ref()),
                });
            }
        }
    }
    Ok((params, has_variadic))
}

fn walk_result_simple_or_params(
    simple: Option<&cst::SimpleType<'_>>,
    params: Option<&cst::ParameterList<'_>>,
    src: &[u8],
) -> R<Vec<ParamDecl>> {
    if let Some(st) = simple {
        let ty = walk_simple_type(st, src)?;
        Ok(vec![ParamDecl {
            names: vec![],
            ty,
            variadic: false,
            span: sp(st),
        }])
    } else if let Some(pl) = params {
        let (params, _) = walk_param_list(pl, src)?;
        Ok(params)
    } else {
        Ok(vec![])
    }
}

fn walk_opt_result_func_decl(
    result: &Option<cst::FunctionDeclarationResult<'_>>,
    src: &[u8],
) -> R<Vec<ParamDecl>> {
    match result {
        None => Ok(vec![]),
        Some(cst::FunctionDeclarationResult::SimpleType(st)) => {
            walk_result_simple_or_params(Some(st), None, src)
        }
        Some(cst::FunctionDeclarationResult::ParameterList(pl)) => {
            walk_result_simple_or_params(None, Some(pl), src)
        }
    }
}

fn walk_opt_result_method_decl(
    result: &Option<cst::MethodDeclarationResult<'_>>,
    src: &[u8],
) -> R<Vec<ParamDecl>> {
    match result {
        None => Ok(vec![]),
        Some(cst::MethodDeclarationResult::SimpleType(st)) => {
            walk_result_simple_or_params(Some(st), None, src)
        }
        Some(cst::MethodDeclarationResult::ParameterList(pl)) => {
            walk_result_simple_or_params(None, Some(pl), src)
        }
    }
}

fn walk_opt_result_func_lit(
    result: &Option<cst::FuncLiteralResult<'_>>,
    src: &[u8],
) -> R<Vec<ParamDecl>> {
    match result {
        None => Ok(vec![]),
        Some(cst::FuncLiteralResult::SimpleType(st)) => {
            walk_result_simple_or_params(Some(st), None, src)
        }
        Some(cst::FuncLiteralResult::ParameterList(pl)) => {
            walk_result_simple_or_params(None, Some(pl), src)
        }
    }
}

fn walk_opt_result_func_type(
    result: &Option<cst::FunctionTypeResult<'_>>,
    src: &[u8],
) -> R<Vec<ParamDecl>> {
    match result {
        None => Ok(vec![]),
        Some(cst::FunctionTypeResult::SimpleType(st)) => {
            walk_result_simple_or_params(Some(st), None, src)
        }
        Some(cst::FunctionTypeResult::ParameterList(pl)) => {
            walk_result_simple_or_params(None, Some(pl), src)
        }
    }
}

fn walk_opt_result_method_elem(
    result: &Option<cst::MethodElemResult<'_>>,
    src: &[u8],
) -> R<Vec<ParamDecl>> {
    match result {
        None => Ok(vec![]),
        Some(cst::MethodElemResult::SimpleType(st)) => {
            walk_result_simple_or_params(Some(st), None, src)
        }
        Some(cst::MethodElemResult::ParameterList(pl)) => {
            walk_result_simple_or_params(None, Some(pl), src)
        }
    }
}

// --- Type Parameters ---

fn walk_opt_type_param_list(
    tpl: &Option<cst::TypeParameterList<'_>>,
    src: &[u8],
) -> R<Vec<TypeParam>> {
    let Some(tpl) = tpl else { return Ok(vec![]) };
    tpl.children
        .iter()
        .map(|tpd| walk_type_param_decl(tpd, src))
        .collect()
}

fn walk_type_param_decl(tpd: &cst::TypeParameterDeclaration<'_>, src: &[u8]) -> R<TypeParam> {
    let names: Vec<Ident> = tpd.name.iter().map(|n| ident_from_id(n)).collect();
    let constraint = walk_type_constraint(&tpd.r#type, src)?;
    Ok(TypeParam {
        names,
        constraint,
        span: sp(tpd),
    })
}

fn walk_type_constraint(tc: &cst::TypeConstraint<'_>, src: &[u8]) -> R<TypeExpr> {
    if tc.children.len() == 1 {
        walk_type(&tc.children[0], src)
    } else {
        let terms: R<Vec<_>> = tc
            .children
            .iter()
            .map(|t| {
                Ok(TypeTermElem {
                    tilde: false,
                    ty: walk_type(t, src)?,
                    span: sp(t),
                })
            })
            .collect();
        Ok(TypeExpr::Interface(InterfaceType {
            elements: vec![InterfaceElem::TypeTerm(TypeTerm {
                terms: terms?,
                span: sp(tc),
            })],
            span: sp(tc),
        }))
    }
}

// --- Types ---

fn walk_type(t: &cst::Type<'_>, src: &[u8]) -> R<TypeExpr> {
    match t {
        cst::Type::SimpleType(st) => walk_simple_type(st, src),
        cst::Type::ParenthesizedType(pt) => walk_type(&pt.children, src),
    }
}

fn walk_simple_type(st: &cst::SimpleType<'_>, src: &[u8]) -> R<TypeExpr> {
    match st {
        cst::SimpleType::TypeIdentifier(id) => Ok(TypeExpr::Named(Ident {
            name: id.text().to_owned(),
            span: sp(id.as_ref()),
        })),
        cst::SimpleType::QualifiedType(qt) => Ok(TypeExpr::Qualified {
            package: Ident {
                name: qt.package.text().to_owned(),
                span: sp(&qt.package),
            },
            name: Ident {
                name: qt.name.text().to_owned(),
                span: sp(&qt.name),
            },
        }),
        cst::SimpleType::PointerType(pt) => {
            Ok(TypeExpr::Pointer(Box::new(walk_type(&pt.children, src)?)))
        }
        cst::SimpleType::ArrayType(at) => {
            let len = walk_expr(&at.length, src)?;
            let elem = walk_type(&at.element, src)?;
            Ok(TypeExpr::Array {
                len: Box::new(len),
                elem: Box::new(elem),
            })
        }
        cst::SimpleType::SliceType(sl) => {
            Ok(TypeExpr::Slice(Box::new(walk_type(&sl.element, src)?)))
        }
        cst::SimpleType::MapType(mt) => Ok(TypeExpr::Map {
            key: Box::new(walk_type(&mt.key, src)?),
            value: Box::new(walk_type(&mt.value, src)?),
        }),
        cst::SimpleType::ChannelType(ct) => walk_channel_type(ct, src),
        cst::SimpleType::StructType(st) => Ok(TypeExpr::Struct(walk_struct_type(st, src)?)),
        cst::SimpleType::InterfaceType(it) => {
            Ok(TypeExpr::Interface(walk_interface_type(it, src)?))
        }
        cst::SimpleType::FunctionType(ft) => {
            let (params, _) = walk_param_list(&ft.parameters, src)?;
            let results = walk_opt_result_func_type(&ft.result, src)?;
            Ok(TypeExpr::Func(FuncType {
                type_params: vec![],
                params,
                results,
                span: sp(ft.as_ref()),
            }))
        }
        cst::SimpleType::GenericType(gt) => {
            let base = walk_generic_type_base(&gt.r#type, src)?;
            let args = walk_type_arguments(&gt.type_arguments, src)?;
            Ok(TypeExpr::Generic {
                base: Box::new(base),
                args,
            })
        }
        cst::SimpleType::NegatedType(nt) => walk_type(&nt.children, src),
    }
}

fn walk_generic_type_base(gt: &cst::GenericTypeType<'_>, src: &[u8]) -> R<TypeExpr> {
    match gt {
        cst::GenericTypeType::TypeIdentifier(id) => Ok(TypeExpr::Named(Ident {
            name: id.text().to_owned(),
            span: sp(id.as_ref()),
        })),
        cst::GenericTypeType::QualifiedType(qt) => Ok(TypeExpr::Qualified {
            package: Ident {
                name: qt.package.text().to_owned(),
                span: sp(&qt.package),
            },
            name: Ident {
                name: qt.name.text().to_owned(),
                span: sp(&qt.name),
            },
        }),
        cst::GenericTypeType::NegatedType(nt) => walk_type(&nt.children, src),
    }
}

fn walk_type_arguments(ta: &cst::TypeArguments<'_>, src: &[u8]) -> R<Vec<TypeExpr>> {
    ta.children
        .iter()
        .map(|te| {
            if te.children.len() == 1 {
                walk_type(&te.children[0], src)
            } else {
                let terms: R<Vec<_>> = te
                    .children
                    .iter()
                    .map(|t| {
                        Ok(TypeTermElem {
                            tilde: false,
                            ty: walk_type(t, src)?,
                            span: sp(t),
                        })
                    })
                    .collect();
                Ok(TypeExpr::Interface(InterfaceType {
                    elements: vec![InterfaceElem::TypeTerm(TypeTerm {
                        terms: terms?,
                        span: sp(te),
                    })],
                    span: sp(te),
                }))
            }
        })
        .collect()
}

fn walk_channel_type(ct: &cst::ChannelType<'_>, src: &[u8]) -> R<TypeExpr> {
    let elem = walk_type(&ct.value, src)?;
    let start = ct.span.start_byte;
    let src_slice = &src[start..];
    let direction = if src_slice.starts_with(b"<-chan") {
        ChanDir::Recv
    } else if src_slice.starts_with(b"chan<-") || src_slice.starts_with(b"chan <-") {
        ChanDir::Send
    } else {
        ChanDir::Both
    };
    Ok(TypeExpr::Channel {
        direction,
        elem: Box::new(elem),
    })
}

fn walk_struct_type(st: &cst::StructType<'_>, src: &[u8]) -> R<StructType> {
    let mut fields = Vec::new();
    for fd in &st.children.children {
        fields.push(walk_field_decl(fd, src)?);
    }
    Ok(StructType {
        fields,
        span: sp(st),
    })
}

fn walk_field_decl(fd: &cst::FieldDeclaration<'_>, src: &[u8]) -> R<FieldDecl> {
    let tag = fd.tag.as_ref().map(|t| {
        let (raw, fspan) = match t {
            cst::FieldDeclarationTag::InterpretedStringLiteral(s) => {
                // Can't call .text() on InterpretedStringLiteral; use span position
                // We'll reconstruct later. For now store empty and fix with src access.
                (String::new(), sp(s.as_ref()))
            }
            cst::FieldDeclarationTag::RawStringLiteral(s) => (String::new(), sp(s.as_ref())),
        };
        StringLit { raw, span: fspan }
    });

    if fd.name.is_empty() {
        let ty = walk_field_decl_type(&fd.r#type, src)?;
        Ok(FieldDecl::Embedded {
            ty,
            tag,
            span: sp(fd),
        })
    } else {
        let names: Vec<Ident> = fd
            .name
            .iter()
            .map(|n| Ident {
                name: n.text().to_owned(),
                span: sp(n),
            })
            .collect();
        let ty = walk_field_decl_type(&fd.r#type, src)?;
        Ok(FieldDecl::Named {
            names,
            ty,
            tag,
            span: sp(fd),
        })
    }
}

fn walk_field_decl_type(t: &cst::FieldDeclarationType<'_>, src: &[u8]) -> R<TypeExpr> {
    match t {
        cst::FieldDeclarationType::Type(ty) => walk_type(ty, src),
        cst::FieldDeclarationType::TypeIdentifier(id) => Ok(TypeExpr::Named(Ident {
            name: id.text().to_owned(),
            span: sp(id.as_ref()),
        })),
        cst::FieldDeclarationType::QualifiedType(qt) => Ok(TypeExpr::Qualified {
            package: Ident {
                name: qt.package.text().to_owned(),
                span: sp(&qt.package),
            },
            name: Ident {
                name: qt.name.text().to_owned(),
                span: sp(&qt.name),
            },
        }),
        cst::FieldDeclarationType::GenericType(gt) => {
            let base = walk_generic_type_base(&gt.r#type, src)?;
            let args = walk_type_arguments(&gt.type_arguments, src)?;
            Ok(TypeExpr::Generic {
                base: Box::new(base),
                args,
            })
        }
    }
}

fn walk_interface_type(it: &cst::InterfaceType<'_>, src: &[u8]) -> R<InterfaceType> {
    let mut elements = Vec::new();
    for child in &it.children {
        match child {
            cst::InterfaceTypeChildren::MethodElem(me) => {
                let name = Ident {
                    name: me.name.text().to_owned(),
                    span: sp(&me.name),
                };
                let (params, _) = walk_param_list(&me.parameters, src)?;
                let results = walk_opt_result_method_elem(&me.result, src)?;
                elements.push(InterfaceElem::Method {
                    name,
                    ty: FuncType {
                        type_params: vec![],
                        params,
                        results,
                        span: sp(me.as_ref()),
                    },
                    span: sp(me.as_ref()),
                });
            }
            cst::InterfaceTypeChildren::TypeElem(te) => {
                if te.children.len() == 1 {
                    elements.push(InterfaceElem::Embedded(walk_type(&te.children[0], src)?));
                } else {
                    let terms: R<Vec<_>> = te
                        .children
                        .iter()
                        .map(|t| {
                            Ok(TypeTermElem {
                                tilde: false,
                                ty: walk_type(t, src)?,
                                span: sp(t),
                            })
                        })
                        .collect();
                    elements.push(InterfaceElem::TypeTerm(TypeTerm {
                        terms: terms?,
                        span: sp(te.as_ref()),
                    }));
                }
            }
        }
    }
    Ok(InterfaceType {
        elements,
        span: sp(it),
    })
}

// --- Blocks and Statements ---

fn walk_block(b: &cst::Block<'_>, src: &[u8]) -> R<Block> {
    let stmts = match &b.children {
        Some(sl) => sl
            .children
            .iter()
            .map(|s| walk_stmt(s, src))
            .collect::<R<Vec<_>>>()?,
        None => vec![],
    };
    Ok(Block { stmts, span: sp(b) })
}

fn walk_stmt(s: &cst::Statement<'_>, src: &[u8]) -> R<Stmt> {
    match s {
        cst::Statement::SimpleStatement(ss) => walk_simple_stmt(ss, src),
        cst::Statement::Block(b) => Ok(Stmt::Block(walk_block(b, src)?)),
        cst::Statement::ReturnStatement(rs) => {
            let values = rs
                .children
                .as_ref()
                .map(|el| walk_expr_list(el, src))
                .transpose()?
                .unwrap_or_default();
            Ok(Stmt::Return {
                values,
                span: sp(rs.as_ref()),
            })
        }
        cst::Statement::IfStatement(is) => walk_if_stmt(is, src),
        cst::Statement::ForStatement(fs) => walk_for_stmt(fs, src),
        cst::Statement::ExpressionSwitchStatement(ess) => walk_expr_switch(ess, src),
        cst::Statement::TypeSwitchStatement(tss) => walk_type_switch(tss, src),
        cst::Statement::SelectStatement(ss) => walk_select(ss, src),
        cst::Statement::GoStatement(gs) => {
            Ok(Stmt::Go(walk_expr(&gs.children, src)?, sp(gs.as_ref())))
        }
        cst::Statement::DeferStatement(ds) => {
            Ok(Stmt::Defer(walk_expr(&ds.children, src)?, sp(ds.as_ref())))
        }
        cst::Statement::BreakStatement(bs) => {
            let label = bs.children.as_ref().map(|l| Ident {
                name: l.text().to_owned(),
                span: sp(l),
            });
            Ok(Stmt::Break(label, sp(bs.as_ref())))
        }
        cst::Statement::ContinueStatement(cs) => {
            let label = cs.children.as_ref().map(|l| Ident {
                name: l.text().to_owned(),
                span: sp(l),
            });
            Ok(Stmt::Continue(label, sp(cs.as_ref())))
        }
        cst::Statement::GotoStatement(gs) => Ok(Stmt::Goto(
            Ident {
                name: gs.children.text().to_owned(),
                span: sp(&gs.children),
            },
            sp(gs.as_ref()),
        )),
        cst::Statement::FallthroughStatement(fs) => Ok(Stmt::Fallthrough(sp(fs.as_ref()))),
        cst::Statement::EmptyStatement(es) => Ok(Stmt::Empty(sp(es.as_ref()))),
        cst::Statement::LabeledStatement(ls) => {
            let label = Ident {
                name: ls.label.text().to_owned(),
                span: sp(&ls.label),
            };
            let body = ls
                .children
                .as_ref()
                .map(|s| walk_stmt(s, src))
                .transpose()?
                .unwrap_or(Stmt::Empty(sp(ls.as_ref())));
            Ok(Stmt::Labeled {
                label,
                body: Box::new(body),
                span: sp(ls.as_ref()),
            })
        }
        cst::Statement::TypeDeclaration(td) => {
            let specs = walk_type_decl(td, src)?;
            if let Some(first) = specs.into_iter().next() {
                Ok(Stmt::TypeDecl(first, sp(td.as_ref())))
            } else {
                Ok(Stmt::Empty(sp(td.as_ref())))
            }
        }
        cst::Statement::VarDeclaration(vd) => {
            let specs = walk_var_decl(vd, src)?;
            if let Some(first) = specs.into_iter().next() {
                Ok(Stmt::VarDecl(first, sp(vd.as_ref())))
            } else {
                Ok(Stmt::Empty(sp(vd.as_ref())))
            }
        }
        cst::Statement::ConstDeclaration(cd) => {
            let specs = walk_const_decl(cd, src)?;
            if let Some(first) = specs.into_iter().next() {
                Ok(Stmt::ConstDecl(first, sp(cd.as_ref())))
            } else {
                Ok(Stmt::Empty(sp(cd.as_ref())))
            }
        }
    }
}

fn walk_simple_stmt(ss: &cst::SimpleStatement<'_>, src: &[u8]) -> R<Stmt> {
    match ss {
        cst::SimpleStatement::AssignmentStatement(asg) => walk_assign_stmt(asg, src),
        cst::SimpleStatement::ShortVarDeclaration(svd) => walk_short_var_decl(svd, src),
        cst::SimpleStatement::IncStatement(is) => {
            Ok(Stmt::Inc(walk_expr(&is.children, src)?, sp(is.as_ref())))
        }
        cst::SimpleStatement::DecStatement(ds) => {
            Ok(Stmt::Dec(walk_expr(&ds.children, src)?, sp(ds.as_ref())))
        }
        cst::SimpleStatement::SendStatement(ss) => Ok(Stmt::Send {
            channel: walk_expr(&ss.channel, src)?,
            value: walk_expr(&ss.value, src)?,
            span: sp(ss.as_ref()),
        }),
        cst::SimpleStatement::ExpressionStatement(es) => {
            Ok(Stmt::Expr(walk_expr(&es.children, src)?, sp(es.as_ref())))
        }
    }
}

fn walk_assign_stmt(asg: &cst::AssignmentStatement<'_>, src: &[u8]) -> R<Stmt> {
    let lhs = walk_expr_list(&asg.left, src)?;
    let rhs = walk_expr_list(&asg.right, src)?;
    let op = match &asg.operator {
        cst::AssignmentStatementOperator::Eq(_) => AssignOp::Assign,
        cst::AssignmentStatementOperator::PlusEq(_) => AssignOp::AddAssign,
        cst::AssignmentStatementOperator::MinusEq(_) => AssignOp::SubAssign,
        cst::AssignmentStatementOperator::StarEq(_) => AssignOp::MulAssign,
        cst::AssignmentStatementOperator::SlashEq(_) => AssignOp::DivAssign,
        cst::AssignmentStatementOperator::PercentEq(_) => AssignOp::RemAssign,
        cst::AssignmentStatementOperator::AmpEq(_) => AssignOp::AndAssign,
        cst::AssignmentStatementOperator::PipeEq(_) => AssignOp::OrAssign,
        cst::AssignmentStatementOperator::CaretEq(_) => AssignOp::XorAssign,
        cst::AssignmentStatementOperator::AmpCaretEq(_) => AssignOp::AndNotAssign,
        cst::AssignmentStatementOperator::ShlEq(_) => AssignOp::ShlAssign,
        cst::AssignmentStatementOperator::ShrEq(_) => AssignOp::ShrAssign,
    };
    Ok(Stmt::Assign {
        lhs,
        op,
        rhs,
        span: sp(asg),
    })
}

fn walk_short_var_decl(svd: &cst::ShortVarDeclaration<'_>, src: &[u8]) -> R<Stmt> {
    let names: Vec<Ident> = svd
        .left
        .children
        .iter()
        .filter_map(|e| {
            if let cst::Expression::Identifier(id) = e {
                Some(ident_from_id(id))
            } else {
                None
            }
        })
        .collect();
    let values = walk_expr_list(&svd.right, src)?;
    Ok(Stmt::ShortVarDecl {
        names,
        values,
        span: sp(svd),
    })
}

fn walk_if_stmt(is: &cst::IfStatement<'_>, src: &[u8]) -> R<Stmt> {
    let init = is
        .initializer
        .as_ref()
        .map(|s| walk_simple_stmt(s, src))
        .transpose()?
        .map(Box::new);
    let cond = walk_expr(&is.condition, src)?;
    let body = walk_block(&is.consequence, src)?;
    let else_: Option<Box<Stmt>> = is
        .alternative
        .as_ref()
        .map(|a| -> R<Box<Stmt>> {
            match a {
                cst::IfStatementAlternative::Block(b) => {
                    Ok(Box::new(Stmt::Block(walk_block(b, src)?)))
                }
                cst::IfStatementAlternative::IfStatement(is2) => {
                    Ok(Box::new(walk_if_stmt(is2, src)?))
                }
            }
        })
        .transpose()?;
    Ok(Stmt::If {
        init,
        cond,
        body,
        else_,
        span: sp(is),
    })
}

fn walk_for_stmt(fs: &cst::ForStatement<'_>, src: &[u8]) -> R<Stmt> {
    match &fs.children {
        None => Ok(Stmt::For {
            init: None,
            cond: None,
            post: None,
            body: walk_block(&fs.body, src)?,
            span: sp(fs),
        }),
        Some(cst::ForStatementChildren::Expression(expr)) => Ok(Stmt::For {
            init: None,
            cond: Some(walk_expr(expr, src)?),
            post: None,
            body: walk_block(&fs.body, src)?,
            span: sp(fs),
        }),
        Some(cst::ForStatementChildren::ForClause(fc)) => {
            let init = fc
                .initializer
                .as_ref()
                .map(|s| walk_simple_stmt(s, src))
                .transpose()?
                .map(Box::new);
            let cond = fc
                .condition
                .as_ref()
                .map(|e| walk_expr(e, src))
                .transpose()?;
            let post = fc
                .update
                .as_ref()
                .map(|s| walk_simple_stmt(s, src))
                .transpose()?
                .map(Box::new);
            Ok(Stmt::For {
                init,
                cond,
                post,
                body: walk_block(&fs.body, src)?,
                span: sp(fs),
            })
        }
        Some(cst::ForStatementChildren::RangeClause(rc)) => {
            walk_range_stmt(rc, &fs.body, sp(fs), src)
        }
    }
}

fn walk_range_stmt(
    rc: &cst::RangeClause<'_>,
    body: &cst::Block<'_>,
    span: Span,
    src: &[u8],
) -> R<Stmt> {
    let (key, value, assign) = if let Some(left) = &rc.left {
        let exprs = walk_expr_list(left, src)?;
        let key = exprs.first().cloned();
        let value = exprs.get(1).cloned();
        let left_end = left.span().end_byte;
        let right_start = rc.right.span().start_byte;
        let between = &src[left_end..right_start];
        let assign = if between.windows(2).any(|w| w == b":=") {
            RangeAssign::Define
        } else {
            RangeAssign::Assign
        };
        (key, value, assign)
    } else {
        (None, None, RangeAssign::Define)
    };
    Ok(Stmt::ForRange {
        key,
        value,
        assign,
        iterable: Box::new(walk_expr(&rc.right, src)?),
        body: walk_block(body, src)?,
        span,
    })
}

fn walk_expr_switch(ess: &cst::ExpressionSwitchStatement<'_>, src: &[u8]) -> R<Stmt> {
    let init = ess
        .initializer
        .as_ref()
        .map(|s| walk_simple_stmt(s, src))
        .transpose()?
        .map(Box::new);
    let tag = ess.value.as_ref().map(|e| walk_expr(e, src)).transpose()?;
    let mut cases = Vec::new();
    for child in &ess.children {
        match child {
            cst::ExpressionSwitchStatementChildren::ExpressionCase(ec) => {
                let exprs = walk_expr_list(&ec.value, src)?;
                let body = walk_opt_stmt_list(&ec.children, src)?;
                cases.push(ExprCase {
                    exprs,
                    body,
                    span: sp(ec.as_ref()),
                });
            }
            cst::ExpressionSwitchStatementChildren::DefaultCase(dc) => {
                let body = walk_opt_stmt_list(&dc.children, src)?;
                cases.push(ExprCase {
                    exprs: vec![],
                    body,
                    span: sp(dc.as_ref()),
                });
            }
        }
    }
    Ok(Stmt::Switch {
        init,
        tag,
        cases,
        span: sp(ess),
    })
}

fn walk_type_switch(tss: &cst::TypeSwitchStatement<'_>, src: &[u8]) -> R<Stmt> {
    let init = tss
        .initializer
        .as_ref()
        .map(|s| walk_simple_stmt(s, src))
        .transpose()?
        .map(Box::new);
    let name = tss.alias.as_ref().and_then(|el| {
        el.children.first().and_then(|e| {
            if let cst::Expression::Identifier(id) = e {
                Some(ident_from_id(id))
            } else {
                None
            }
        })
    });
    let expr = walk_expr(&tss.value, src)?;
    let assign = TypeSwitchAssign {
        name,
        expr,
        span: sp(tss),
    };

    let mut cases = Vec::new();
    for child in &tss.children {
        match child {
            cst::TypeSwitchStatementChildren::TypeCase(tc) => {
                let types: Vec<TypeExpr> = tc
                    .r#type
                    .iter()
                    .filter_map(|t| match t {
                        cst::TypeCaseType::Type(ty) => Some(walk_type(ty, src)),
                        cst::TypeCaseType::Comma(_) => None,
                    })
                    .collect::<R<Vec<_>>>()?;
                let body = walk_opt_stmt_list(&tc.children, src)?;
                cases.push(TypeCase {
                    types,
                    body,
                    span: sp(tc.as_ref()),
                });
            }
            cst::TypeSwitchStatementChildren::DefaultCase(dc) => {
                let body = walk_opt_stmt_list(&dc.children, src)?;
                cases.push(TypeCase {
                    types: vec![],
                    body,
                    span: sp(dc.as_ref()),
                });
            }
        }
    }
    Ok(Stmt::TypeSwitch {
        init,
        assign,
        cases,
        span: sp(tss),
    })
}

fn walk_select(ss: &cst::SelectStatement<'_>, src: &[u8]) -> R<Stmt> {
    let mut cases = Vec::new();
    for child in &ss.children {
        match child {
            cst::SelectStatementChildren::CommunicationCase(cc) => {
                let body = walk_opt_stmt_list(&cc.children, src)?;
                match &cc.communication {
                    cst::CommunicationCaseCommunication::SendStatement(send) => {
                        cases.push(CommCase::Send {
                            stmt: Stmt::Send {
                                channel: walk_expr(&send.channel, src)?,
                                value: walk_expr(&send.value, src)?,
                                span: sp(send.as_ref()),
                            },
                            body,
                            span: sp(cc.as_ref()),
                        });
                    }
                    cst::CommunicationCaseCommunication::ReceiveStatement(recv) => {
                        let recv_expr = walk_expr(&recv.right, src)?;
                        let stmt = if let Some(left) = &recv.left {
                            let lhs = walk_expr_list(left, src)?;
                            let left_end = left.span().end_byte;
                            let right_start = recv.right.span().start_byte;
                            let between = &src[left_end..right_start];
                            if between.windows(2).any(|w| w == b":=") {
                                let names = lhs
                                    .into_iter()
                                    .filter_map(|e| {
                                        if let Expr::Ident(id) = e {
                                            Some(id)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                                Some(Stmt::ShortVarDecl {
                                    names,
                                    values: vec![recv_expr],
                                    span: sp(recv.as_ref()),
                                })
                            } else {
                                Some(Stmt::Assign {
                                    lhs,
                                    op: AssignOp::Assign,
                                    rhs: vec![recv_expr],
                                    span: sp(recv.as_ref()),
                                })
                            }
                        } else {
                            None
                        };
                        cases.push(CommCase::Recv {
                            stmt,
                            body,
                            span: sp(cc.as_ref()),
                        });
                    }
                }
            }
            cst::SelectStatementChildren::DefaultCase(dc) => {
                let body = walk_opt_stmt_list(&dc.children, src)?;
                cases.push(CommCase::Default {
                    body,
                    span: sp(dc.as_ref()),
                });
            }
        }
    }
    Ok(Stmt::Select {
        cases,
        span: sp(ss),
    })
}

fn walk_opt_stmt_list(sl: &Option<cst::StatementList<'_>>, src: &[u8]) -> R<Vec<Stmt>> {
    sl.as_ref()
        .map(|sl| {
            sl.children
                .iter()
                .map(|s| walk_stmt(s, src))
                .collect::<R<Vec<_>>>()
        })
        .transpose()
        .map(|v| v.unwrap_or_default())
}

// --- Expressions ---

fn walk_expr_list(el: &cst::ExpressionList<'_>, src: &[u8]) -> R<Vec<Expr>> {
    el.children.iter().map(|e| walk_expr(e, src)).collect()
}

fn walk_expr(e: &cst::Expression<'_>, src: &[u8]) -> R<Expr> {
    match e {
        cst::Expression::Identifier(id) => Ok(Expr::Ident(ident_from_id(id))),
        cst::Expression::IntLiteral(lit) => Ok(Expr::Int(IntLit {
            raw: lit.text().to_owned(),
            span: sp(lit.as_ref()),
        })),
        cst::Expression::FloatLiteral(lit) => Ok(Expr::Float(FloatLit {
            raw: lit.text().to_owned(),
            span: sp(lit.as_ref()),
        })),
        cst::Expression::ImaginaryLiteral(lit) => Ok(Expr::Imaginary(ImaginaryLit {
            raw: lit.text().to_owned(),
            span: sp(lit.as_ref()),
        })),
        cst::Expression::RuneLiteral(lit) => Ok(Expr::Rune(RuneLit {
            raw: lit.text().to_owned(),
            span: sp(lit.as_ref()),
        })),
        cst::Expression::InterpretedStringLiteral(lit) => Ok(Expr::String(StringLit {
            raw: text_from_span(src, &lit.span).to_owned(),
            span: sp(lit.as_ref()),
        })),
        cst::Expression::RawStringLiteral(lit) => Ok(Expr::RawString(RawStringLit {
            raw: text_from_span(src, &lit.span).to_owned(),
            span: sp(lit.as_ref()),
        })),
        cst::Expression::True(t) => Ok(Expr::True(sp(t.as_ref()))),
        cst::Expression::False(f) => Ok(Expr::False(sp(f.as_ref()))),
        cst::Expression::Nil(n) => Ok(Expr::Nil(sp(n.as_ref()))),
        cst::Expression::Iota(i) => Ok(Expr::Iota(sp(i.as_ref()))),
        cst::Expression::ParenthesizedExpression(pe) => Ok(Expr::Paren(
            Box::new(walk_expr(&pe.children, src)?),
            sp(pe.as_ref()),
        )),
        cst::Expression::SelectorExpression(sel) => {
            let operand = walk_expr(&sel.operand, src)?;
            let field = Ident {
                name: sel.field.text().to_owned(),
                span: sp(&sel.field),
            };
            Ok(Expr::Selector {
                operand: Box::new(operand),
                field,
                span: sp(sel.as_ref()),
            })
        }
        cst::Expression::IndexExpression(ie) => Ok(Expr::Index {
            operand: Box::new(walk_expr(&ie.operand, src)?),
            index: Box::new(walk_expr(&ie.index, src)?),
            span: sp(ie.as_ref()),
        }),
        cst::Expression::SliceExpression(se) => Ok(Expr::Slice {
            operand: Box::new(walk_expr(&se.operand, src)?),
            low: se
                .start
                .as_ref()
                .map(|e| walk_expr(e, src))
                .transpose()?
                .map(Box::new),
            high: se
                .end
                .as_ref()
                .map(|e| walk_expr(e, src))
                .transpose()?
                .map(Box::new),
            max: se
                .capacity
                .as_ref()
                .map(|e| walk_expr(e, src))
                .transpose()?
                .map(Box::new),
            span: sp(se.as_ref()),
        }),
        cst::Expression::CallExpression(ce) => {
            let func = walk_expr(&ce.function, src)?;
            let type_args = ce
                .type_arguments
                .as_ref()
                .map(|ta| walk_type_arguments(ta, src))
                .transpose()?
                .unwrap_or_default();
            let mut args = Vec::new();
            let mut ellipsis = false;
            for child in &ce.arguments.children {
                match child {
                    cst::ArgumentListChildren::Expression(expr) => {
                        args.push(walk_expr(expr, src)?);
                    }
                    cst::ArgumentListChildren::VariadicArgument(va) => {
                        ellipsis = true;
                        args.push(walk_expr(&va.children, src)?);
                    }
                    cst::ArgumentListChildren::Type(ty) => {
                        let type_expr = walk_type(ty, src)?;
                        args.push(type_to_expr(&type_expr, cvt_span(&ty.span())));
                    }
                }
            }
            Ok(Expr::Call {
                func: Box::new(func),
                type_args,
                args,
                ellipsis,
                span: sp(ce.as_ref()),
            })
        }
        cst::Expression::TypeConversionExpression(tce) => {
            let ty = walk_type(&tce.r#type, src)?;
            let func = type_to_expr(&ty, sp(tce.as_ref()));
            let operand = walk_expr(&tce.operand, src)?;
            Ok(Expr::Call {
                func: Box::new(func),
                type_args: vec![],
                args: vec![operand],
                ellipsis: false,
                span: sp(tce.as_ref()),
            })
        }
        cst::Expression::TypeAssertionExpression(tae) => Ok(Expr::TypeAssert {
            operand: Box::new(walk_expr(&tae.operand, src)?),
            ty: Box::new(walk_type(&tae.r#type, src)?),
            span: sp(tae.as_ref()),
        }),
        cst::Expression::TypeInstantiationExpression(tie) => {
            let base = walk_type(&tie.r#type, src)?;
            let args: Vec<TypeExpr> = tie
                .children
                .iter()
                .map(|t| walk_type(t, src))
                .collect::<R<_>>()?;
            let ty = TypeExpr::Generic {
                base: Box::new(base),
                args,
            };
            Ok(type_to_expr(&ty, sp(tie.as_ref())))
        }
        cst::Expression::UnaryExpression(ue) => {
            let operand = walk_expr(&ue.operand, src)?;
            let op = match &ue.operator {
                cst::UnaryExpressionOperator::Bang(_) => UnaryOp::Not,
                cst::UnaryExpressionOperator::Minus(_) => UnaryOp::Neg,
                cst::UnaryExpressionOperator::Plus(_) => UnaryOp::Pos,
                cst::UnaryExpressionOperator::Star(_) => UnaryOp::Deref,
                cst::UnaryExpressionOperator::Amp(_) => UnaryOp::Addr,
                cst::UnaryExpressionOperator::LArrow(_) => UnaryOp::Recv,
                cst::UnaryExpressionOperator::Caret(_) => UnaryOp::BitNot,
            };
            Ok(Expr::Unary {
                op,
                operand: Box::new(operand),
                span: sp(ue.as_ref()),
            })
        }
        cst::Expression::BinaryExpression(be) => {
            let left = walk_expr(&be.left, src)?;
            let right = walk_expr(&be.right, src)?;
            let op = match &be.operator {
                cst::BinaryExpressionOperator::Plus(_) => BinaryOp::Add,
                cst::BinaryExpressionOperator::Minus(_) => BinaryOp::Sub,
                cst::BinaryExpressionOperator::Star(_) => BinaryOp::Mul,
                cst::BinaryExpressionOperator::Slash(_) => BinaryOp::Div,
                cst::BinaryExpressionOperator::Percent(_) => BinaryOp::Rem,
                cst::BinaryExpressionOperator::Amp(_) => BinaryOp::And,
                cst::BinaryExpressionOperator::Pipe(_) => BinaryOp::Or,
                cst::BinaryExpressionOperator::Caret(_) => BinaryOp::Xor,
                cst::BinaryExpressionOperator::AmpCaret(_) => BinaryOp::AndNot,
                cst::BinaryExpressionOperator::Shl(_) => BinaryOp::Shl,
                cst::BinaryExpressionOperator::Shr(_) => BinaryOp::Shr,
                cst::BinaryExpressionOperator::AmpAmp(_) => BinaryOp::LogAnd,
                cst::BinaryExpressionOperator::PipePipe(_) => BinaryOp::LogOr,
                cst::BinaryExpressionOperator::EqEq(_) => BinaryOp::Eq,
                cst::BinaryExpressionOperator::NotEq(_) => BinaryOp::Ne,
                cst::BinaryExpressionOperator::Lt(_) => BinaryOp::Lt,
                cst::BinaryExpressionOperator::LtEq(_) => BinaryOp::Le,
                cst::BinaryExpressionOperator::Gt(_) => BinaryOp::Gt,
                cst::BinaryExpressionOperator::GtEq(_) => BinaryOp::Ge,
            };
            Ok(Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                span: sp(be.as_ref()),
            })
        }
        cst::Expression::CompositeLiteral(cl) => {
            let ty = walk_composite_lit_type(&cl.r#type, src)?;
            let elems = walk_literal_value(&cl.body, src)?;
            Ok(Expr::Composite {
                ty: Box::new(ty),
                elems,
                span: sp(cl.as_ref()),
            })
        }
        cst::Expression::FuncLiteral(fl) => {
            let (params, _) = walk_param_list(&fl.parameters, src)?;
            let results = walk_opt_result_func_lit(&fl.result, src)?;
            let body = walk_block(&fl.body, src)?;
            Ok(Expr::FuncLit {
                ty: FuncType {
                    type_params: vec![],
                    params,
                    results,
                    span: sp(fl.as_ref()),
                },
                body,
                span: sp(fl.as_ref()),
            })
        }
    }
}

fn walk_composite_lit_type(t: &cst::CompositeLiteralType<'_>, src: &[u8]) -> R<TypeExpr> {
    match t {
        cst::CompositeLiteralType::TypeIdentifier(id) => Ok(TypeExpr::Named(Ident {
            name: id.text().to_owned(),
            span: sp(id.as_ref()),
        })),
        cst::CompositeLiteralType::QualifiedType(qt) => Ok(TypeExpr::Qualified {
            package: Ident {
                name: qt.package.text().to_owned(),
                span: sp(&qt.package),
            },
            name: Ident {
                name: qt.name.text().to_owned(),
                span: sp(&qt.name),
            },
        }),
        cst::CompositeLiteralType::ArrayType(at) => Ok(TypeExpr::Array {
            len: Box::new(walk_expr(&at.length, src)?),
            elem: Box::new(walk_type(&at.element, src)?),
        }),
        cst::CompositeLiteralType::SliceType(sl) => {
            Ok(TypeExpr::Slice(Box::new(walk_type(&sl.element, src)?)))
        }
        cst::CompositeLiteralType::MapType(mt) => Ok(TypeExpr::Map {
            key: Box::new(walk_type(&mt.key, src)?),
            value: Box::new(walk_type(&mt.value, src)?),
        }),
        cst::CompositeLiteralType::StructType(st) => {
            Ok(TypeExpr::Struct(walk_struct_type(st, src)?))
        }
        cst::CompositeLiteralType::ImplicitLengthArrayType(ila) => Ok(TypeExpr::Array {
            len: Box::new(Expr::Ident(Ident {
                name: "...".to_owned(),
                span: sp(ila.as_ref()),
            })),
            elem: Box::new(walk_type(&ila.element, src)?),
        }),
        cst::CompositeLiteralType::GenericType(gt) => {
            let base = walk_generic_type_base(&gt.r#type, src)?;
            let args = walk_type_arguments(&gt.type_arguments, src)?;
            Ok(TypeExpr::Generic {
                base: Box::new(base),
                args,
            })
        }
    }
}

fn walk_literal_value(lv: &cst::LiteralValue<'_>, src: &[u8]) -> R<Vec<KeyedElem>> {
    let mut elems = Vec::new();
    for child in &lv.children {
        match child {
            cst::LiteralValueChildren::KeyedElement(ke) => {
                let key = walk_literal_elem(&ke.key, src)?;
                let value = walk_literal_elem(&ke.value, src)?;
                elems.push(KeyedElem {
                    key: Some(key),
                    value,
                    span: sp(ke.as_ref()),
                });
            }
            cst::LiteralValueChildren::LiteralElement(le) => {
                let value = walk_literal_elem(le, src)?;
                elems.push(KeyedElem {
                    key: None,
                    value,
                    span: sp(le.as_ref()),
                });
            }
        }
    }
    Ok(elems)
}

fn walk_literal_elem(le: &cst::LiteralElement<'_>, src: &[u8]) -> R<Expr> {
    match &le.children {
        cst::LiteralElementChildren::Expression(expr) => walk_expr(expr, src),
        cst::LiteralElementChildren::LiteralValue(lv) => {
            let elems = walk_literal_value(lv, src)?;
            Ok(Expr::Composite {
                ty: Box::new(TypeExpr::Named(Ident::synthetic(""))),
                elems,
                span: sp(lv.as_ref()),
            })
        }
    }
}

// --- Helpers ---

fn ident_from_id(id: &cst::Identifier<'_>) -> Ident {
    Ident {
        name: id.text().to_owned(),
        span: sp(id),
    }
}

fn type_to_expr(ty: &TypeExpr, span: Span) -> Expr {
    match ty {
        TypeExpr::Named(id) => Expr::Ident(id.clone()),
        TypeExpr::Qualified { package, name } => Expr::Qualified {
            package: package.clone(),
            name: name.clone(),
            span,
        },
        _ => Expr::Ident(Ident {
            name: crate::printer::Printer::type_expr(ty),
            span,
        }),
    }
}

/// Parse a Go source file into a go-model SourceFile.
pub fn parse_and_walk(src: &[u8]) -> Result<SourceFile, WalkError> {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .map_err(|_| WalkError::UnexpectedNode {
            kind: "failed to set language".into(),
        })?;
    let tree = parser.parse(src, None).ok_or(WalkError::UnexpectedNode {
        kind: "parse returned None".into(),
    })?;
    let cst_file = cst::SourceFile::from_node(tree.root_node(), src)?;
    walk_source_file(&cst_file, src)
}

/// Check if the tree-sitter parse had errors.
pub fn parse_has_error(src: &[u8]) -> bool {
    let mut parser = tree_sitter::Parser::new();
    // Language set failure would indicate a build misconfiguration, not a runtime error.
    if parser
        .set_language(&tree_sitter_go::LANGUAGE.into())
        .is_err()
    {
        return true;
    }
    tree_sitter_has_error(&mut parser, src)
}

fn tree_sitter_has_error(parser: &mut tree_sitter::Parser, src: &[u8]) -> bool {
    parser
        .parse(src, None)
        .is_none_or(|t| t.root_node().has_error())
}

#[cfg(test)]
mod tests;
