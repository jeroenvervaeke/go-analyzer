use serde::{Deserialize, Serialize};

use crate::{Block, Expr, Ident, Span, StringLit, TypeExpr};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncType {
    pub type_params: Vec<TypeParam>,
    pub params: Vec<ParamDecl>,
    pub results: Vec<ParamDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamDecl {
    pub names: Vec<Ident>,
    pub ty: TypeExpr,
    pub variadic: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeParam {
    pub names: Vec<Ident>,
    pub constraint: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receiver {
    pub name: Option<Ident>,
    pub type_params: Vec<TypeParam>,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FuncDecl {
    pub name: Ident,
    pub ty: FuncType,
    pub body: Option<Block>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodDecl {
    pub receiver: Receiver,
    pub name: Ident,
    pub ty: FuncType,
    pub body: Option<Block>,
    pub doc: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StructType {
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldDecl {
    Named {
        names: Vec<Ident>,
        ty: TypeExpr,
        tag: Option<StringLit>,
        span: Span,
    },
    Embedded {
        ty: TypeExpr,
        tag: Option<StringLit>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceType {
    pub elements: Vec<InterfaceElem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterfaceElem {
    Method {
        name: Ident,
        ty: FuncType,
        span: Span,
    },
    TypeTerm(TypeTerm),
    Embedded(TypeExpr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTerm {
    pub terms: Vec<TypeTermElem>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TypeTermElem {
    pub tilde: bool,
    pub ty: TypeExpr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TypeSpec {
    Alias {
        name: Ident,
        type_params: Vec<TypeParam>,
        ty: TypeExpr,
        span: Span,
    },
    Def {
        name: Ident,
        type_params: Vec<TypeParam>,
        ty: TypeExpr,
        span: Span,
    },
}

impl TypeSpec {
    pub fn name(&self) -> &Ident {
        match self {
            Self::Alias { name, .. } | Self::Def { name, .. } => name,
        }
    }

    pub fn span(&self) -> Span {
        match self {
            Self::Alias { span, .. } | Self::Def { span, .. } => *span,
        }
    }

    pub fn ty(&self) -> &TypeExpr {
        match self {
            Self::Alias { ty, .. } | Self::Def { ty, .. } => ty,
        }
    }

    pub fn is_struct(&self) -> bool {
        matches!(
            self,
            Self::Def {
                ty: TypeExpr::Struct(_),
                ..
            }
        )
    }

    pub fn is_interface(&self) -> bool {
        matches!(
            self,
            Self::Def {
                ty: TypeExpr::Interface(_),
                ..
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarSpec {
    pub names: Vec<Ident>,
    pub ty: Option<TypeExpr>,
    pub values: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConstSpec {
    pub names: Vec<Ident>,
    pub ty: Option<TypeExpr>,
    pub values: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportSpec {
    pub alias: ImportAlias,
    pub path: StringLit,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImportAlias {
    Implicit,
    Dot,
    Blank,
    Named(Ident),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceFile {
    pub package: Ident,
    pub imports: Vec<ImportSpec>,
    pub decls: Vec<TopLevelDecl>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TopLevelDecl {
    Func(Box<FuncDecl>),
    Method(Box<MethodDecl>),
    Type(Vec<TypeSpec>),
    Var(Vec<VarSpec>),
    Const(Vec<ConstSpec>),
}
