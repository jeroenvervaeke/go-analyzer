use go_model::*;

pub(crate) struct Printer;

fn indent_str(depth: usize) -> String {
    "\t".repeat(depth)
}

impl Printer {
    pub fn type_expr(t: &TypeExpr) -> String {
        Self::type_expr_at(t, 0)
    }

    fn type_expr_at(t: &TypeExpr, indent: usize) -> String {
        match t {
            TypeExpr::Named(id) => id.name.clone(),
            TypeExpr::Qualified { package, name } => {
                format!("{}.{}", package.name, name.name)
            }
            TypeExpr::Pointer(inner) => format!("*{}", Self::type_expr_at(inner, indent)),
            TypeExpr::Array { len, elem } => {
                format!("[{}]{}", Self::expr(len), Self::type_expr_at(elem, indent))
            }
            TypeExpr::Slice(elem) => format!("[]{}", Self::type_expr_at(elem, indent)),
            TypeExpr::Map { key, value } => {
                format!(
                    "map[{}]{}",
                    Self::type_expr_at(key, indent),
                    Self::type_expr_at(value, indent)
                )
            }
            TypeExpr::Channel { direction, elem } => {
                let elem_str = Self::type_expr_at(elem, indent);
                match direction {
                    ChanDir::Both => format!("chan {elem_str}"),
                    ChanDir::Recv => format!("<-chan {elem_str}"),
                    ChanDir::Send => format!("chan<- {elem_str}"),
                }
            }
            TypeExpr::Func(ft) => Self::func_type_at(ft, indent),
            TypeExpr::Interface(it) => Self::interface_type_at(it, indent),
            TypeExpr::Struct(st) => Self::struct_type_at(st, indent),
            TypeExpr::Generic { base, args } => {
                let args_str: Vec<_> = args.iter().map(|a| Self::type_expr_at(a, indent)).collect();
                format!(
                    "{}[{}]",
                    Self::type_expr_at(base, indent),
                    args_str.join(", ")
                )
            }
        }
    }

    fn func_type_at(ft: &FuncType, indent: usize) -> String {
        let mut s = String::from("func");
        if !ft.type_params.is_empty() {
            s.push('[');
            s.push_str(&Self::type_params(&ft.type_params));
            s.push(']');
        }
        s.push('(');
        s.push_str(&Self::params_at(&ft.params, indent));
        s.push(')');
        Self::append_results_at(&mut s, &ft.results, indent);
        s
    }

    fn type_params(tps: &[TypeParam]) -> String {
        tps.iter()
            .map(|tp| {
                let names: Vec<_> = tp.names.iter().map(|n| n.name.as_str()).collect();
                format!("{} {}", names.join(", "), Self::type_expr(&tp.constraint))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn params(params: &[ParamDecl]) -> String {
        Self::params_at(params, 0)
    }

    fn params_at(params: &[ParamDecl], indent: usize) -> String {
        params
            .iter()
            .map(|p| {
                let mut s = String::new();
                if !p.names.is_empty() {
                    let names: Vec<_> = p.names.iter().map(|n| n.name.as_str()).collect();
                    s.push_str(&names.join(", "));
                    s.push(' ');
                }
                if p.variadic {
                    s.push_str("...");
                }
                s.push_str(&Self::type_expr_at(&p.ty, indent));
                s
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn append_results(s: &mut String, results: &[ParamDecl]) {
        Self::append_results_at(s, results, 0);
    }

    fn append_results_at(s: &mut String, results: &[ParamDecl], indent: usize) {
        if results.is_empty() {
            return;
        }
        // Single unnamed result — no parens needed
        if results.len() == 1 && results[0].names.is_empty() {
            s.push(' ');
            s.push_str(&Self::type_expr_at(&results[0].ty, indent));
            return;
        }
        s.push_str(" (");
        s.push_str(&Self::params_at(results, indent));
        s.push(')');
    }

    fn struct_type_at(st: &StructType, indent: usize) -> String {
        if st.fields.is_empty() {
            return "struct{}".to_owned();
        }
        let inner = indent + 1;
        let mut s = String::from("struct {\n");
        for f in &st.fields {
            s.push_str(&indent_str(inner));
            match f {
                FieldDecl::Named { names, ty, tag, .. } => {
                    let names_str: Vec<_> = names.iter().map(|n| n.name.as_str()).collect();
                    s.push_str(&names_str.join(", "));
                    s.push(' ');
                    s.push_str(&Self::type_expr_at(ty, inner));
                    if let Some(tag) = tag {
                        s.push(' ');
                        s.push_str(&tag.raw);
                    }
                }
                FieldDecl::Embedded { ty, tag, .. } => {
                    s.push_str(&Self::type_expr_at(ty, inner));
                    if let Some(tag) = tag {
                        s.push(' ');
                        s.push_str(&tag.raw);
                    }
                }
            }
            s.push('\n');
        }
        s.push_str(&indent_str(indent));
        s.push('}');
        s
    }

    fn interface_type_at(it: &InterfaceType, indent: usize) -> String {
        if it.elements.is_empty() {
            return "interface{}".to_owned();
        }
        let inner = indent + 1;
        let mut s = String::from("interface {\n");
        for elem in &it.elements {
            s.push_str(&indent_str(inner));
            match elem {
                InterfaceElem::Method { name, ty, .. } => {
                    s.push_str(&name.name);
                    s.push('(');
                    s.push_str(&Self::params_at(&ty.params, inner));
                    s.push(')');
                    Self::append_results_at(&mut s, &ty.results, inner);
                }
                InterfaceElem::TypeTerm(tt) => {
                    let terms: Vec<_> = tt
                        .terms
                        .iter()
                        .map(|t| {
                            let mut ts = String::new();
                            if t.tilde {
                                ts.push('~');
                            }
                            ts.push_str(&Self::type_expr_at(&t.ty, inner));
                            ts
                        })
                        .collect();
                    s.push_str(&terms.join(" | "));
                }
                InterfaceElem::Embedded(ty) => {
                    s.push_str(&Self::type_expr_at(ty, inner));
                }
            }
            s.push('\n');
        }
        s.push_str(&indent_str(indent));
        s.push('}');
        s
    }

    pub fn expr(e: &Expr) -> String {
        Self::expr_at(e, 0)
    }

    fn expr_at(e: &Expr, indent: usize) -> String {
        match e {
            Expr::Ident(id) => id.name.clone(),
            Expr::Qualified { package, name, .. } => format!("{}.{}", package.name, name.name),
            Expr::Int(lit) => lit.raw.clone(),
            Expr::Float(lit) => lit.raw.clone(),
            Expr::Imaginary(lit) => lit.raw.clone(),
            Expr::Rune(lit) => lit.raw.clone(),
            Expr::String(lit) => lit.raw.clone(),
            Expr::RawString(lit) => lit.raw.clone(),
            Expr::True(_) => "true".to_owned(),
            Expr::False(_) => "false".to_owned(),
            Expr::Nil(_) => "nil".to_owned(),
            Expr::Iota(_) => "iota".to_owned(),
            Expr::Paren(inner, _) => format!("({})", Self::expr_at(inner, indent)),
            Expr::Composite { ty, elems, .. } => {
                let ty_str = Self::type_expr_at(ty, indent);
                if elems.is_empty() {
                    return format!("{ty_str}{{}}");
                }
                let elems_str: Vec<_> = elems
                    .iter()
                    .map(|e| {
                        if let Some(key) = &e.key {
                            format!(
                                "{}: {}",
                                Self::expr_at(key, indent),
                                Self::expr_at(&e.value, indent)
                            )
                        } else {
                            Self::expr_at(&e.value, indent)
                        }
                    })
                    .collect();
                format!("{ty_str}{{{}}}", elems_str.join(", "))
            }
            Expr::FuncLit { ty, body, .. } => {
                let mut s = Self::func_type_at(ty, indent);
                s.push(' ');
                s.push_str(&Self::block_at(body, indent));
                s
            }
            Expr::Call {
                func,
                type_args,
                args,
                ellipsis,
                ..
            } => {
                let mut s = Self::expr_prec_at(func, 100, indent);
                if !type_args.is_empty() {
                    let ta: Vec<_> = type_args
                        .iter()
                        .map(|a| Self::type_expr_at(a, indent))
                        .collect();
                    s.push('[');
                    s.push_str(&ta.join(", "));
                    s.push(']');
                }
                s.push('(');
                let args_str: Vec<_> = args.iter().map(|a| Self::expr_at(a, indent)).collect();
                s.push_str(&args_str.join(", "));
                if *ellipsis {
                    s.push_str("...");
                }
                s.push(')');
                s
            }
            Expr::Selector { operand, field, .. } => {
                format!(
                    "{}.{}",
                    Self::expr_prec_at(operand, 100, indent),
                    field.name
                )
            }
            Expr::Index { operand, index, .. } => {
                format!(
                    "{}[{}]",
                    Self::expr_prec_at(operand, 100, indent),
                    Self::expr_at(index, indent)
                )
            }
            Expr::Slice {
                operand,
                low,
                high,
                max,
                ..
            } => {
                let op_str = Self::expr_prec_at(operand, 100, indent);
                let low_str = low
                    .as_ref()
                    .map_or(String::new(), |e| Self::expr_at(e, indent));
                let high_str = high
                    .as_ref()
                    .map_or(String::new(), |e| Self::expr_at(e, indent));
                if let Some(max) = max {
                    format!(
                        "{op_str}[{low_str}:{high_str}:{}]",
                        Self::expr_at(max, indent)
                    )
                } else {
                    format!("{op_str}[{low_str}:{high_str}]")
                }
            }
            Expr::TypeAssert { operand, ty, .. } => {
                format!(
                    "{}.({})",
                    Self::expr_prec_at(operand, 100, indent),
                    Self::type_expr_at(ty, indent)
                )
            }
            Expr::Unary { op, operand, .. } => {
                let op_str = match op {
                    UnaryOp::Not => "!",
                    UnaryOp::Neg => "-",
                    UnaryOp::Pos => "+",
                    UnaryOp::Deref => "*",
                    UnaryOp::Addr => "&",
                    UnaryOp::Recv => "<-",
                    UnaryOp::BitNot => "^",
                };
                format!("{op_str}{}", Self::expr_at(operand, indent))
            }
            Expr::Binary {
                op, left, right, ..
            } => {
                let op_str = Self::binary_op_str(*op);
                let left_str = Self::expr_maybe_parens_at(left, *op, true, indent);
                let right_str = Self::expr_maybe_parens_at(right, *op, false, indent);
                format!("{left_str} {op_str} {right_str}")
            }
        }
    }

    fn expr_prec_at(e: &Expr, _parent_prec: u8, indent: usize) -> String {
        Self::expr_at(e, indent)
    }

    fn expr_maybe_parens_at(
        child: &Expr,
        parent_op: BinaryOp,
        is_left: bool,
        indent: usize,
    ) -> String {
        if Self::needs_parens(parent_op, child, is_left) {
            format!("({})", Self::expr_at(child, indent))
        } else {
            Self::expr_at(child, indent)
        }
    }

    /// Returns true when a child expression inside a binary parent needs parentheses.
    /// A child needs parens if:
    /// - It's a Binary with lower precedence than the parent
    /// - It's a Binary with equal precedence on the right side (to preserve left-associativity)
    pub fn needs_parens(parent_op: BinaryOp, child: &Expr, is_left: bool) -> bool {
        let Expr::Binary { op: child_op, .. } = child else {
            return false;
        };
        let parent_prec = parent_op.precedence();
        let child_prec = child_op.precedence();

        if child_prec < parent_prec {
            return true;
        }
        // Same precedence on right side needs parens for left-associativity
        if child_prec == parent_prec && !is_left {
            return true;
        }
        false
    }

    fn binary_op_str(op: BinaryOp) -> &'static str {
        match op {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Rem => "%",
            BinaryOp::And => "&",
            BinaryOp::Or => "|",
            BinaryOp::Xor => "^",
            BinaryOp::AndNot => "&^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::LogAnd => "&&",
            BinaryOp::LogOr => "||",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
        }
    }

    // Part of the printer's public API surface; currently only exercised
    // by tests but intentionally kept for future callers.
    #[allow(dead_code)]
    pub fn stmt(s: &Stmt) -> String {
        Self::stmt_at(s, 0)
    }

    fn stmt_at(s: &Stmt, indent: usize) -> String {
        match s {
            Stmt::Empty(_) => String::new(),
            Stmt::Block(b) => Self::block_at(b, indent),
            Stmt::Expr(e, _) => Self::expr_at(e, indent),
            Stmt::Assign { lhs, op, rhs, .. } => {
                let lhs_str: Vec<_> = lhs.iter().map(|e| Self::expr_at(e, indent)).collect();
                let rhs_str: Vec<_> = rhs.iter().map(|e| Self::expr_at(e, indent)).collect();
                let op_str = Self::assign_op_str(*op);
                format!("{} {} {}", lhs_str.join(", "), op_str, rhs_str.join(", "))
            }
            Stmt::ShortVarDecl { names, values, .. } => {
                let names_str: Vec<_> = names.iter().map(|n| n.name.as_str()).collect();
                let values_str: Vec<_> = values.iter().map(|e| Self::expr_at(e, indent)).collect();
                format!("{} := {}", names_str.join(", "), values_str.join(", "))
            }
            Stmt::VarDecl(vs, _) => Self::var_spec_at(vs, indent),
            Stmt::ConstDecl(cs, _) => Self::const_spec_at(cs, indent),
            Stmt::Inc(e, _) => format!("{}++", Self::expr_at(e, indent)),
            Stmt::Dec(e, _) => format!("{}--", Self::expr_at(e, indent)),
            Stmt::Send { channel, value, .. } => {
                format!(
                    "{} <- {}",
                    Self::expr_at(channel, indent),
                    Self::expr_at(value, indent)
                )
            }
            Stmt::Return { values, .. } => {
                if values.is_empty() {
                    "return".to_owned()
                } else {
                    let vals: Vec<_> = values.iter().map(|e| Self::expr_at(e, indent)).collect();
                    format!("return {}", vals.join(", "))
                }
            }
            Stmt::If {
                init,
                cond,
                body,
                else_,
                ..
            } => {
                let mut out = String::from("if ");
                if let Some(init) = init {
                    out.push_str(&Self::stmt_at(init, indent));
                    out.push_str("; ");
                }
                out.push_str(&Self::expr_at(cond, indent));
                out.push(' ');
                out.push_str(&Self::block_at(body, indent));
                if let Some(else_) = else_ {
                    out.push_str(" else ");
                    out.push_str(&Self::stmt_at(else_, indent));
                }
                out
            }
            Stmt::For {
                init,
                cond,
                post,
                body,
                ..
            } => {
                let mut out = String::from("for ");
                if init.is_some() || post.is_some() {
                    if let Some(init) = init {
                        out.push_str(&Self::stmt_at(init, indent));
                    }
                    out.push_str("; ");
                    if let Some(cond) = cond {
                        out.push_str(&Self::expr_at(cond, indent));
                    }
                    out.push_str("; ");
                    if let Some(post) = post {
                        out.push_str(&Self::stmt_at(post, indent));
                    }
                    out.push(' ');
                } else if let Some(cond) = cond {
                    out.push_str(&Self::expr_at(cond, indent));
                    out.push(' ');
                }
                out.push_str(&Self::block_at(body, indent));
                out
            }
            Stmt::ForRange {
                key,
                value,
                assign,
                iterable,
                body,
                ..
            } => {
                let mut out = String::from("for ");
                let has_vars = key.is_some() || value.is_some();
                if has_vars {
                    if let Some(key) = key {
                        out.push_str(&Self::expr_at(key, indent));
                    } else {
                        out.push('_');
                    }
                    if let Some(value) = value {
                        out.push_str(", ");
                        out.push_str(&Self::expr_at(value, indent));
                    }
                    match assign {
                        RangeAssign::Define => out.push_str(" := "),
                        RangeAssign::Assign => out.push_str(" = "),
                    }
                }
                out.push_str("range ");
                out.push_str(&Self::expr_at(iterable, indent));
                out.push(' ');
                out.push_str(&Self::block_at(body, indent));
                out
            }
            Stmt::Switch {
                init, tag, cases, ..
            } => {
                let mut out = String::from("switch ");
                if let Some(init) = init {
                    out.push_str(&Self::stmt_at(init, indent));
                    out.push_str("; ");
                }
                if let Some(tag) = tag {
                    out.push_str(&Self::expr_at(tag, indent));
                    out.push(' ');
                }
                out.push_str("{\n");
                for case in cases {
                    out.push_str(&indent_str(indent));
                    if case.exprs.is_empty() {
                        out.push_str("default:\n");
                    } else {
                        let exprs: Vec<_> = case
                            .exprs
                            .iter()
                            .map(|e| Self::expr_at(e, indent))
                            .collect();
                        out.push_str(&format!("case {}:\n", exprs.join(", ")));
                    }
                    for stmt in &case.body {
                        out.push_str(&indent_str(indent + 1));
                        out.push_str(&Self::stmt_at(stmt, indent + 1));
                        out.push('\n');
                    }
                }
                out.push_str(&indent_str(indent));
                out.push('}');
                out
            }
            Stmt::TypeSwitch {
                init,
                assign,
                cases,
                ..
            } => {
                let mut out = String::from("switch ");
                if let Some(init) = init {
                    out.push_str(&Self::stmt_at(init, indent));
                    out.push_str("; ");
                }
                if let Some(name) = &assign.name {
                    out.push_str(&name.name);
                    out.push_str(" := ");
                }
                out.push_str(&Self::expr_at(&assign.expr, indent));
                out.push_str(".(type) {\n");
                for case in cases {
                    out.push_str(&indent_str(indent));
                    if case.types.is_empty() {
                        out.push_str("default:\n");
                    } else {
                        let types: Vec<_> = case
                            .types
                            .iter()
                            .map(|t| Self::type_expr_at(t, indent))
                            .collect();
                        out.push_str(&format!("case {}:\n", types.join(", ")));
                    }
                    for stmt in &case.body {
                        out.push_str(&indent_str(indent + 1));
                        out.push_str(&Self::stmt_at(stmt, indent + 1));
                        out.push('\n');
                    }
                }
                out.push_str(&indent_str(indent));
                out.push('}');
                out
            }
            Stmt::Select { cases, .. } => {
                let mut out = String::from("select {\n");
                for case in cases {
                    match case {
                        CommCase::Send { stmt, body, .. } => {
                            out.push_str(&indent_str(indent));
                            out.push_str(&format!("case {}:\n", Self::stmt_at(stmt, indent)));
                            for st in body {
                                out.push_str(&indent_str(indent + 1));
                                out.push_str(&Self::stmt_at(st, indent + 1));
                                out.push('\n');
                            }
                        }
                        CommCase::Recv { stmt, body, .. } => {
                            out.push_str(&indent_str(indent));
                            if let Some(stmt) = stmt {
                                out.push_str(&format!("case {}:\n", Self::stmt_at(stmt, indent)));
                            } else {
                                out.push_str("case:\n");
                            }
                            for st in body {
                                out.push_str(&indent_str(indent + 1));
                                out.push_str(&Self::stmt_at(st, indent + 1));
                                out.push('\n');
                            }
                        }
                        CommCase::Default { body, .. } => {
                            out.push_str(&indent_str(indent));
                            out.push_str("default:\n");
                            for st in body {
                                out.push_str(&indent_str(indent + 1));
                                out.push_str(&Self::stmt_at(st, indent + 1));
                                out.push('\n');
                            }
                        }
                    }
                }
                out.push_str(&indent_str(indent));
                out.push('}');
                out
            }
            Stmt::Go(e, _) => format!("go {}", Self::expr_at(e, indent)),
            Stmt::Defer(e, _) => format!("defer {}", Self::expr_at(e, indent)),
            Stmt::Break(label, _) => {
                if let Some(l) = label {
                    format!("break {}", l.name)
                } else {
                    "break".to_owned()
                }
            }
            Stmt::Continue(label, _) => {
                if let Some(l) = label {
                    format!("continue {}", l.name)
                } else {
                    "continue".to_owned()
                }
            }
            Stmt::Goto(label, _) => format!("goto {}", label.name),
            Stmt::Fallthrough(_) => "fallthrough".to_owned(),
            Stmt::Labeled { label, body, .. } => {
                format!(
                    "{}:\n{}{}",
                    label.name,
                    indent_str(indent),
                    Self::stmt_at(body, indent)
                )
            }
            Stmt::TypeDecl(ts, _) => format!("type {}", Self::type_spec_inner_at(ts, indent)),
        }
    }

    fn var_spec_at(vs: &VarSpec, indent: usize) -> String {
        let names: Vec<_> = vs.names.iter().map(|n| n.name.as_str()).collect();
        let mut s = format!("var {}", names.join(", "));
        if let Some(ty) = &vs.ty {
            s.push(' ');
            s.push_str(&Self::type_expr_at(ty, indent));
        }
        if !vs.values.is_empty() {
            let vals: Vec<_> = vs.values.iter().map(|e| Self::expr_at(e, indent)).collect();
            s.push_str(" = ");
            s.push_str(&vals.join(", "));
        }
        s
    }

    fn const_spec_at(cs: &ConstSpec, indent: usize) -> String {
        let names: Vec<_> = cs.names.iter().map(|n| n.name.as_str()).collect();
        let mut s = format!("const {}", names.join(", "));
        if let Some(ty) = &cs.ty {
            s.push(' ');
            s.push_str(&Self::type_expr_at(ty, indent));
        }
        if !cs.values.is_empty() {
            let vals: Vec<_> = cs.values.iter().map(|e| Self::expr_at(e, indent)).collect();
            s.push_str(" = ");
            s.push_str(&vals.join(", "));
        }
        s
    }

    fn assign_op_str(op: AssignOp) -> &'static str {
        match op {
            AssignOp::Assign => "=",
            AssignOp::AddAssign => "+=",
            AssignOp::SubAssign => "-=",
            AssignOp::MulAssign => "*=",
            AssignOp::DivAssign => "/=",
            AssignOp::RemAssign => "%=",
            AssignOp::AndAssign => "&=",
            AssignOp::OrAssign => "|=",
            AssignOp::XorAssign => "^=",
            AssignOp::AndNotAssign => "&^=",
            AssignOp::ShlAssign => "<<=",
            AssignOp::ShrAssign => ">>=",
        }
    }

    fn block_at(b: &Block, indent: usize) -> String {
        if b.stmts.is_empty() {
            return "{}".to_owned();
        }
        let inner = indent + 1;
        let mut s = String::from("{\n");
        for stmt in &b.stmts {
            s.push_str(&indent_str(inner));
            s.push_str(&Self::stmt_at(stmt, inner));
            s.push('\n');
        }
        s.push_str(&indent_str(indent));
        s.push('}');
        s
    }

    pub fn func_decl(f: &FuncDecl) -> String {
        let mut s = String::from("func ");
        s.push_str(&f.name.name);
        if !f.ty.type_params.is_empty() {
            s.push('[');
            s.push_str(&Self::type_params(&f.ty.type_params));
            s.push(']');
        }
        s.push('(');
        s.push_str(&Self::params(&f.ty.params));
        s.push(')');
        Self::append_results(&mut s, &f.ty.results);
        if let Some(body) = &f.body {
            s.push(' ');
            s.push_str(&Self::block_at(body, 0));
        }
        s
    }

    pub fn method_decl(m: &MethodDecl) -> String {
        let mut s = String::from("func ");
        // receiver
        s.push('(');
        if let Some(name) = &m.receiver.name {
            s.push_str(&name.name);
            s.push(' ');
        }
        s.push_str(&Self::type_expr(&m.receiver.ty));
        s.push_str(") ");
        s.push_str(&m.name.name);
        if !m.ty.type_params.is_empty() {
            s.push('[');
            s.push_str(&Self::type_params(&m.ty.type_params));
            s.push(']');
        }
        s.push('(');
        s.push_str(&Self::params(&m.ty.params));
        s.push(')');
        Self::append_results(&mut s, &m.ty.results);
        if let Some(body) = &m.body {
            s.push(' ');
            s.push_str(&Self::block_at(body, 0));
        }
        s
    }

    #[cfg(test)]
    pub fn type_spec(t: &TypeSpec) -> String {
        format!("type {}", Self::type_spec_inner_at(t, 0))
    }

    fn type_spec_inner_at(t: &TypeSpec, indent: usize) -> String {
        match t {
            TypeSpec::Alias {
                name,
                type_params,
                ty,
                ..
            } => {
                let mut s = name.name.clone();
                if !type_params.is_empty() {
                    s.push('[');
                    s.push_str(&Self::type_params(type_params));
                    s.push(']');
                }
                s.push_str(" = ");
                s.push_str(&Self::type_expr_at(ty, indent));
                s
            }
            TypeSpec::Def {
                name,
                type_params,
                ty,
                ..
            } => {
                let mut s = name.name.clone();
                if !type_params.is_empty() {
                    s.push('[');
                    s.push_str(&Self::type_params(type_params));
                    s.push(']');
                }
                s.push(' ');
                s.push_str(&Self::type_expr_at(ty, indent));
                s
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use go_model::build;

    /// Assert that gofmt accepts the output when wrapped in a package + function context.
    fn assert_gofmt_valid(go_src: &str) {
        assert_gofmt_valid_raw(&format!("package p\n\n{go_src}\n"));
    }

    fn assert_gofmt_valid_raw(go_src: &str) {
        use std::io::Write;
        use std::process::Command;
        let mut child = Command::new("gofmt")
            .arg("-e")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("gofmt not found");
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(go_src.as_bytes()).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "gofmt rejected:\n---\n{go_src}\n---\nstderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// Wrap a statement in a function body for gofmt validation.
    fn assert_stmt_gofmt_valid(stmt_src: &str) {
        assert_gofmt_valid(&format!("func f() {{\n{stmt_src}\n}}"));
    }

    /// Wrap an expression in a function body as statement for gofmt validation.
    fn assert_expr_gofmt_valid(expr_src: &str) {
        assert_stmt_gofmt_valid(&format!("_ = {expr_src}"));
    }

    /// Wrap a type expr in a valid type declaration for gofmt validation.
    fn assert_type_expr_gofmt_valid(type_src: &str) {
        assert_gofmt_valid(&format!("type T {type_src}"));
    }

    // --- Type expression tests ---

    #[test]
    fn test_print_named_type() {
        let s = Printer::type_expr(&TypeExpr::Named(Ident::synthetic("int")));
        assert_eq!(s, "int");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_qualified_type() {
        let s = Printer::type_expr(&TypeExpr::Qualified {
            package: Ident::synthetic("fmt"),
            name: Ident::synthetic("Stringer"),
        });
        assert_eq!(s, "fmt.Stringer");
    }

    #[test]
    fn test_print_pointer_type() {
        let s = Printer::type_expr(&build::pointer(build::named("int")));
        assert_eq!(s, "*int");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_slice_type() {
        let s = Printer::type_expr(&build::slice(build::named("byte")));
        assert_eq!(s, "[]byte");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_array_type() {
        let s = Printer::type_expr(&TypeExpr::Array {
            len: Box::new(Expr::Int(IntLit {
                raw: "10".into(),
                span: Span::synthetic(),
            })),
            elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
        });
        assert_eq!(s, "[10]int");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_map_type() {
        let s = Printer::type_expr(&build::map_type(
            build::named("string"),
            build::named("int"),
        ));
        assert_eq!(s, "map[string]int");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_chan_types() {
        let both = Printer::type_expr(&TypeExpr::Channel {
            direction: ChanDir::Both,
            elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
        });
        assert_eq!(both, "chan int");
        assert_type_expr_gofmt_valid(&both);

        let recv = Printer::type_expr(&TypeExpr::Channel {
            direction: ChanDir::Recv,
            elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
        });
        assert_eq!(recv, "<-chan int");
        assert_type_expr_gofmt_valid(&recv);

        let send = Printer::type_expr(&TypeExpr::Channel {
            direction: ChanDir::Send,
            elem: Box::new(TypeExpr::Named(Ident::synthetic("int"))),
        });
        assert_eq!(send, "chan<- int");
        assert_type_expr_gofmt_valid(&send);
    }

    #[test]
    fn test_print_func_type() {
        let ft = FuncType {
            type_params: vec![],
            params: vec![ParamDecl {
                names: vec![Ident::synthetic("x")],
                ty: TypeExpr::Named(Ident::synthetic("int")),
                variadic: false,
                span: Span::synthetic(),
            }],
            results: vec![ParamDecl {
                names: vec![],
                ty: TypeExpr::Named(Ident::synthetic("error")),
                variadic: false,
                span: Span::synthetic(),
            }],
            span: Span::synthetic(),
        };
        let s = Printer::type_expr(&TypeExpr::Func(ft));
        assert_eq!(s, "func(x int) error");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_struct_type() {
        let st = StructType {
            fields: vec![FieldDecl::Named {
                names: vec![Ident::synthetic("Name")],
                ty: TypeExpr::Named(Ident::synthetic("string")),
                tag: None,
                span: Span::synthetic(),
            }],
            span: Span::synthetic(),
        };
        let s = Printer::type_expr(&TypeExpr::Struct(st));
        assert!(s.contains("struct"));
        assert!(s.contains("Name string"));
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_empty_struct() {
        let st = StructType {
            fields: vec![],
            span: Span::synthetic(),
        };
        let s = Printer::type_expr(&TypeExpr::Struct(st));
        assert_eq!(s, "struct{}");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_interface_type() {
        let it = InterfaceType {
            elements: vec![InterfaceElem::Method {
                name: Ident::synthetic("String"),
                ty: FuncType {
                    type_params: vec![],
                    params: vec![],
                    results: vec![ParamDecl {
                        names: vec![],
                        ty: TypeExpr::Named(Ident::synthetic("string")),
                        variadic: false,
                        span: Span::synthetic(),
                    }],
                    span: Span::synthetic(),
                },
                span: Span::synthetic(),
            }],
            span: Span::synthetic(),
        };
        let s = Printer::type_expr(&TypeExpr::Interface(it));
        assert!(s.contains("String() string"));
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_empty_interface() {
        let it = InterfaceType {
            elements: vec![],
            span: Span::synthetic(),
        };
        let s = Printer::type_expr(&TypeExpr::Interface(it));
        assert_eq!(s, "interface{}");
        assert_type_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_generic_type() {
        let s = Printer::type_expr(&TypeExpr::Generic {
            base: Box::new(TypeExpr::Named(Ident::synthetic("List"))),
            args: vec![TypeExpr::Named(Ident::synthetic("int"))],
        });
        assert_eq!(s, "List[int]");
    }

    // --- Expression tests ---

    #[test]
    fn test_print_ident_expr() {
        let s = Printer::expr(&build::ident("x"));
        assert_eq!(s, "x");
    }

    #[test]
    fn test_print_int_expr() {
        let s = Printer::expr(&build::int(42));
        assert_eq!(s, "42");
        assert_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_string_expr() {
        let s = Printer::expr(&build::string("hello"));
        assert_eq!(s, "\"hello\"");
        assert_expr_gofmt_valid(&s);
    }

    #[test]
    fn test_print_bool_nil_iota() {
        assert_eq!(Printer::expr(&Expr::True(Span::synthetic())), "true");
        assert_eq!(Printer::expr(&Expr::False(Span::synthetic())), "false");
        assert_eq!(Printer::expr(&Expr::Nil(Span::synthetic())), "nil");
        assert_eq!(Printer::expr(&Expr::Iota(Span::synthetic())), "iota");
    }

    #[test]
    fn test_print_call_expr() {
        let e = build::call(
            build::selector(build::ident("fmt"), "Println"),
            vec![build::string("hello")],
        );
        let s = Printer::expr(&e);
        assert_eq!(s, "fmt.Println(\"hello\")");
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_selector_expr() {
        let s = Printer::expr(&build::selector(build::ident("x"), "Field"));
        assert_eq!(s, "x.Field");
    }

    #[test]
    fn test_print_unary_ops() {
        let deref = Printer::expr(&build::deref(build::ident("x")));
        assert_eq!(deref, "*x");
        let addr = Printer::expr(&build::addr(build::ident("x")));
        assert_eq!(addr, "&x");
        let neg = Printer::expr(&Expr::Unary {
            op: UnaryOp::Neg,
            operand: Box::new(build::int(1)),
            span: Span::synthetic(),
        });
        assert_eq!(neg, "-1");
        let not = Printer::expr(&Expr::Unary {
            op: UnaryOp::Not,
            operand: Box::new(build::ident("x")),
            span: Span::synthetic(),
        });
        assert_eq!(not, "!x");
        let bitnot = Printer::expr(&Expr::Unary {
            op: UnaryOp::BitNot,
            operand: Box::new(build::ident("x")),
            span: Span::synthetic(),
        });
        assert_eq!(bitnot, "^x");
        let recv = Printer::expr(&Expr::Unary {
            op: UnaryOp::Recv,
            operand: Box::new(build::ident("ch")),
            span: Span::synthetic(),
        });
        assert_eq!(recv, "<-ch");
    }

    #[test]
    fn test_print_all_binary_ops() {
        let ops = [
            (BinaryOp::Add, "+"),
            (BinaryOp::Sub, "-"),
            (BinaryOp::Mul, "*"),
            (BinaryOp::Div, "/"),
            (BinaryOp::Rem, "%"),
            (BinaryOp::And, "&"),
            (BinaryOp::Or, "|"),
            (BinaryOp::Xor, "^"),
            (BinaryOp::AndNot, "&^"),
            (BinaryOp::Shl, "<<"),
            (BinaryOp::Shr, ">>"),
            (BinaryOp::LogAnd, "&&"),
            (BinaryOp::LogOr, "||"),
            (BinaryOp::Eq, "=="),
            (BinaryOp::Ne, "!="),
            (BinaryOp::Lt, "<"),
            (BinaryOp::Le, "<="),
            (BinaryOp::Gt, ">"),
            (BinaryOp::Ge, ">="),
        ];
        for (op, expected_str) in ops {
            let e = Expr::Binary {
                op,
                left: Box::new(build::ident("a")),
                right: Box::new(build::ident("b")),
                span: Span::synthetic(),
            };
            let s = Printer::expr(&e);
            assert_eq!(s, format!("a {expected_str} b"), "op: {expected_str}");
            assert_expr_gofmt_valid(&s);
        }
    }

    #[test]
    fn test_print_paren_expr() {
        let e = Expr::Paren(Box::new(build::ident("x")), Span::synthetic());
        assert_eq!(Printer::expr(&e), "(x)");
    }

    #[test]
    fn test_print_index_expr() {
        let e = Expr::Index {
            operand: Box::new(build::ident("arr")),
            index: Box::new(build::int(0)),
            span: Span::synthetic(),
        };
        assert_eq!(Printer::expr(&e), "arr[0]");
        assert_expr_gofmt_valid(&Printer::expr(&e));
    }

    #[test]
    fn test_print_slice_expr() {
        let e = Expr::Slice {
            operand: Box::new(build::ident("s")),
            low: Some(Box::new(build::int(1))),
            high: Some(Box::new(build::int(3))),
            max: None,
            span: Span::synthetic(),
        };
        assert_eq!(Printer::expr(&e), "s[1:3]");
        assert_expr_gofmt_valid(&Printer::expr(&e));

        // 3-index slice
        let e2 = Expr::Slice {
            operand: Box::new(build::ident("s")),
            low: Some(Box::new(build::int(1))),
            high: Some(Box::new(build::int(3))),
            max: Some(Box::new(build::int(5))),
            span: Span::synthetic(),
        };
        assert_eq!(Printer::expr(&e2), "s[1:3:5]");
        assert_expr_gofmt_valid(&Printer::expr(&e2));
    }

    #[test]
    fn test_print_type_assert() {
        let e = Expr::TypeAssert {
            operand: Box::new(build::ident("x")),
            ty: Box::new(build::named("int")),
            span: Span::synthetic(),
        };
        assert_eq!(Printer::expr(&e), "x.(int)");
        assert_expr_gofmt_valid(&Printer::expr(&e));
    }

    #[test]
    fn test_print_composite_lit() {
        let e = Expr::Composite {
            ty: Box::new(TypeExpr::Named(Ident::synthetic("Point"))),
            elems: vec![
                KeyedElem {
                    key: Some(build::ident("X")),
                    value: build::int(1),
                    span: Span::synthetic(),
                },
                KeyedElem {
                    key: Some(build::ident("Y")),
                    value: build::int(2),
                    span: Span::synthetic(),
                },
            ],
            span: Span::synthetic(),
        };
        let s = Printer::expr(&e);
        assert_eq!(s, "Point{X: 1, Y: 2}");
        assert_expr_gofmt_valid(&s);
    }

    // --- needs_parens tests ---

    #[test]
    fn test_needs_parens_lower_prec_child() {
        // a + b inside a * context → needs parens
        let child = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(build::ident("a")),
            right: Box::new(build::ident("b")),
            span: Span::synthetic(),
        };
        assert!(Printer::needs_parens(BinaryOp::Mul, &child, true));
        assert!(Printer::needs_parens(BinaryOp::Mul, &child, false));
    }

    #[test]
    fn test_needs_parens_same_prec_left() {
        // a + b on left side of + → no parens (left-associative)
        let child = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(build::ident("a")),
            right: Box::new(build::ident("b")),
            span: Span::synthetic(),
        };
        assert!(!Printer::needs_parens(BinaryOp::Add, &child, true));
    }

    #[test]
    fn test_needs_parens_same_prec_right() {
        // a + b on right side of + → needs parens (preserve left-associativity)
        let child = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(build::ident("a")),
            right: Box::new(build::ident("b")),
            span: Span::synthetic(),
        };
        assert!(Printer::needs_parens(BinaryOp::Add, &child, false));
    }

    #[test]
    fn test_needs_parens_higher_prec_child() {
        // a * b inside a + context → no parens
        let child = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(build::ident("a")),
            right: Box::new(build::ident("b")),
            span: Span::synthetic(),
        };
        assert!(!Printer::needs_parens(BinaryOp::Add, &child, true));
        assert!(!Printer::needs_parens(BinaryOp::Add, &child, false));
    }

    #[test]
    fn test_needs_parens_non_binary_child() {
        let child = build::ident("x");
        assert!(!Printer::needs_parens(BinaryOp::Add, &child, true));
    }

    #[test]
    fn test_needs_parens_exhaustive_precedence_levels() {
        // Test every pair of precedence levels
        let ops_by_prec: Vec<(u8, BinaryOp)> = vec![
            (1, BinaryOp::LogOr),
            (2, BinaryOp::LogAnd),
            (3, BinaryOp::Eq),
            (4, BinaryOp::Add),
            (5, BinaryOp::Mul),
        ];

        for &(parent_prec, parent_op) in &ops_by_prec {
            for &(child_prec, child_op) in &ops_by_prec {
                let child = Expr::Binary {
                    op: child_op,
                    left: Box::new(build::ident("a")),
                    right: Box::new(build::ident("b")),
                    span: Span::synthetic(),
                };

                if child_prec < parent_prec {
                    // Lower-prec child always needs parens
                    assert!(
                        Printer::needs_parens(parent_op, &child, true),
                        "child_prec={child_prec} < parent_prec={parent_prec}, left"
                    );
                    assert!(
                        Printer::needs_parens(parent_op, &child, false),
                        "child_prec={child_prec} < parent_prec={parent_prec}, right"
                    );
                } else if child_prec == parent_prec {
                    // Same prec: no parens on left, parens on right
                    assert!(
                        !Printer::needs_parens(parent_op, &child, true),
                        "child_prec={child_prec} == parent_prec={parent_prec}, left"
                    );
                    assert!(
                        Printer::needs_parens(parent_op, &child, false),
                        "child_prec={child_prec} == parent_prec={parent_prec}, right"
                    );
                } else {
                    // Higher-prec child never needs parens
                    assert!(
                        !Printer::needs_parens(parent_op, &child, true),
                        "child_prec={child_prec} > parent_prec={parent_prec}, left"
                    );
                    assert!(
                        !Printer::needs_parens(parent_op, &child, false),
                        "child_prec={child_prec} > parent_prec={parent_prec}, right"
                    );
                }
            }
        }
    }

    // --- Statement tests ---

    #[test]
    fn test_print_return_stmt() {
        let s = Printer::stmt(&Stmt::Return {
            values: vec![build::ident("x")],
            span: Span::synthetic(),
        });
        assert_eq!(s, "return x");
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_empty_return() {
        let s = Printer::stmt(&Stmt::Return {
            values: vec![],
            span: Span::synthetic(),
        });
        assert_eq!(s, "return");
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_assign_stmt() {
        let s = Printer::stmt(&Stmt::Assign {
            lhs: vec![build::ident("x")],
            op: AssignOp::Assign,
            rhs: vec![build::int(1)],
            span: Span::synthetic(),
        });
        assert_eq!(s, "x = 1");
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_short_var_decl() {
        let s = Printer::stmt(&Stmt::ShortVarDecl {
            names: vec![Ident::synthetic("x")],
            values: vec![build::int(1)],
            span: Span::synthetic(),
        });
        assert_eq!(s, "x := 1");
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_inc_dec() {
        let inc = Printer::stmt(&Stmt::Inc(build::ident("x"), Span::synthetic()));
        assert_eq!(inc, "x++");
        assert_stmt_gofmt_valid(&inc);

        let dec = Printer::stmt(&Stmt::Dec(build::ident("x"), Span::synthetic()));
        assert_eq!(dec, "x--");
        assert_stmt_gofmt_valid(&dec);
    }

    #[test]
    fn test_print_send_stmt() {
        let s = Printer::stmt(&Stmt::Send {
            channel: build::ident("ch"),
            value: build::int(1),
            span: Span::synthetic(),
        });
        assert_eq!(s, "ch <- 1");
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_go_defer() {
        let go = Printer::stmt(&Stmt::Go(
            build::call(build::ident("f"), vec![]),
            Span::synthetic(),
        ));
        assert_eq!(go, "go f()");
        assert_stmt_gofmt_valid(&go);

        let defer = Printer::stmt(&Stmt::Defer(
            build::call(build::ident("f"), vec![]),
            Span::synthetic(),
        ));
        assert_eq!(defer, "defer f()");
        assert_stmt_gofmt_valid(&defer);
    }

    #[test]
    fn test_print_break_continue_goto_fallthrough() {
        assert_eq!(
            Printer::stmt(&Stmt::Break(None, Span::synthetic())),
            "break"
        );
        assert_eq!(
            Printer::stmt(&Stmt::Continue(None, Span::synthetic())),
            "continue"
        );
        assert_eq!(
            Printer::stmt(&Stmt::Break(
                Some(Ident::synthetic("outer")),
                Span::synthetic()
            )),
            "break outer"
        );
        assert_eq!(
            Printer::stmt(&Stmt::Goto(Ident::synthetic("done"), Span::synthetic())),
            "goto done"
        );
        assert_eq!(
            Printer::stmt(&Stmt::Fallthrough(Span::synthetic())),
            "fallthrough"
        );
    }

    #[test]
    fn test_print_if_stmt() {
        let s = Printer::stmt(&Stmt::If {
            init: None,
            cond: build::ident("x"),
            body: Block {
                stmts: vec![Stmt::Return {
                    values: vec![],
                    span: Span::synthetic(),
                }],
                span: Span::synthetic(),
            },
            else_: None,
            span: Span::synthetic(),
        });
        assert!(s.starts_with("if x {"));
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_if_else_stmt() {
        let s = Printer::stmt(&Stmt::If {
            init: None,
            cond: build::ident("x"),
            body: Block {
                stmts: vec![],
                span: Span::synthetic(),
            },
            else_: Some(Box::new(Stmt::Block(Block {
                stmts: vec![],
                span: Span::synthetic(),
            }))),
            span: Span::synthetic(),
        });
        assert!(s.contains("else"));
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_for_loop() {
        // Simple condition-only loop
        let s = Printer::stmt(&Stmt::For {
            init: None,
            cond: Some(build::ident("true")),
            post: None,
            body: Block {
                stmts: vec![],
                span: Span::synthetic(),
            },
            span: Span::synthetic(),
        });
        assert!(s.starts_with("for true {"));
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_for_range() {
        let s = Printer::stmt(&Stmt::ForRange {
            key: Some(build::ident("k")),
            value: Some(build::ident("v")),
            assign: RangeAssign::Define,
            iterable: Box::new(build::ident("m")),
            body: Block {
                stmts: vec![],
                span: Span::synthetic(),
            },
            span: Span::synthetic(),
        });
        assert!(s.contains("k, v := range m"));
        assert_stmt_gofmt_valid(&s);
    }

    #[test]
    fn test_print_var_decl() {
        let s = Printer::stmt(&Stmt::VarDecl(
            VarSpec {
                names: vec![Ident::synthetic("x")],
                ty: Some(TypeExpr::Named(Ident::synthetic("int"))),
                values: vec![],
                span: Span::synthetic(),
            },
            Span::synthetic(),
        ));
        assert_eq!(s, "var x int");
        assert_stmt_gofmt_valid(&s);
    }

    // --- Declaration tests ---

    #[test]
    fn test_print_func_decl() {
        let f = FuncDecl {
            name: Ident::synthetic("main"),
            ty: FuncType {
                type_params: vec![],
                params: vec![],
                results: vec![],
                span: Span::synthetic(),
            },
            body: Some(Block {
                stmts: vec![],
                span: Span::synthetic(),
            }),
            doc: None,
            span: Span::synthetic(),
        };
        let s = Printer::func_decl(&f);
        assert_eq!(s, "func main() {}");
        assert_gofmt_valid(&s);
    }

    #[test]
    fn test_print_func_decl_with_params_and_results() {
        let f = FuncDecl {
            name: Ident::synthetic("Add"),
            ty: FuncType {
                type_params: vec![],
                params: vec![ParamDecl {
                    names: vec![Ident::synthetic("a"), Ident::synthetic("b")],
                    ty: TypeExpr::Named(Ident::synthetic("int")),
                    variadic: false,
                    span: Span::synthetic(),
                }],
                results: vec![ParamDecl {
                    names: vec![],
                    ty: TypeExpr::Named(Ident::synthetic("int")),
                    variadic: false,
                    span: Span::synthetic(),
                }],
                span: Span::synthetic(),
            },
            body: Some(Block {
                stmts: vec![Stmt::Return {
                    values: vec![Expr::Binary {
                        op: BinaryOp::Add,
                        left: Box::new(build::ident("a")),
                        right: Box::new(build::ident("b")),
                        span: Span::synthetic(),
                    }],
                    span: Span::synthetic(),
                }],
                span: Span::synthetic(),
            }),
            doc: None,
            span: Span::synthetic(),
        };
        let s = Printer::func_decl(&f);
        assert_gofmt_valid(&s);
    }

    #[test]
    fn test_print_method_decl() {
        let m = build::method(
            build::pointer_receiver("x", "Foo"),
            "String",
            vec![],
            vec![build::unnamed_param(build::named("string"))],
            build::block(vec![build::ret(vec![build::call(
                build::selector(build::ident("fmt"), "Sprintf"),
                vec![build::string("%+v"), build::deref(build::ident("x"))],
            )])]),
        );
        let s = Printer::method_decl(&m);
        assert!(s.contains("func (x *Foo) String()"));
        assert_gofmt_valid_raw(&format!("package p\n\nimport \"fmt\"\n\n{s}\n"));
    }

    #[test]
    fn test_print_type_spec_def() {
        let ts = TypeSpec::Def {
            name: Ident::synthetic("MyInt"),
            type_params: vec![],
            ty: TypeExpr::Named(Ident::synthetic("int")),
            span: Span::synthetic(),
        };
        let s = Printer::type_spec(&ts);
        assert_eq!(s, "type MyInt int");
        assert_gofmt_valid(&s);
    }

    #[test]
    fn test_print_type_spec_alias() {
        let ts = TypeSpec::Alias {
            name: Ident::synthetic("MyInt"),
            type_params: vec![],
            ty: TypeExpr::Named(Ident::synthetic("int")),
            span: Span::synthetic(),
        };
        let s = Printer::type_spec(&ts);
        assert_eq!(s, "type MyInt = int");
        assert_gofmt_valid(&s);
    }

    #[test]
    fn test_print_variadic_param() {
        let f = FuncDecl {
            name: Ident::synthetic("Printf"),
            ty: FuncType {
                type_params: vec![],
                params: vec![
                    ParamDecl {
                        names: vec![Ident::synthetic("format")],
                        ty: TypeExpr::Named(Ident::synthetic("string")),
                        variadic: false,
                        span: Span::synthetic(),
                    },
                    ParamDecl {
                        names: vec![Ident::synthetic("args")],
                        ty: TypeExpr::Interface(InterfaceType {
                            elements: vec![],
                            span: Span::synthetic(),
                        }),
                        variadic: true,
                        span: Span::synthetic(),
                    },
                ],
                results: vec![],
                span: Span::synthetic(),
            },
            body: Some(Block {
                stmts: vec![],
                span: Span::synthetic(),
            }),
            doc: None,
            span: Span::synthetic(),
        };
        let s = Printer::func_decl(&f);
        assert!(s.contains("...interface{}"));
        assert_gofmt_valid(&s);
    }
}
