mod operation;

use crate::context::ScopedContext;
use crate::*;
pub(crate) use operation::*;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use unstructured::Document;

pub enum Node {
    Expr(Vec<Node>),
    Data(Data),
    Scope(Box<Node>),
    Value(Vec<String>),
    Operation(Operation),
    Filter(Box<(Node, Arc<Filter>, Node)>),
    Function(Box<(Arc<Function>, Node)>),
    Array(Vec<Node>),
    Map(BTreeMap<Document, Node>),
    Empty(),
}

impl fmt::Debug for Node {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Node::Filter(inner) => write!(f, "Node::Filter({:?} | {:?})", inner.0, inner.2),
            Node::Function(inner) => write!(f, "Node::Function({:?})", inner.1),
            Node::Expr(inner) => write!(f, "Node::Expr({:?})", inner),
            Node::Operation(inner) => write!(f, "Node::Operation({:?})", inner),
            Node::Data(inner) => write!(f, "Node::Data({:?})", inner),
            Node::Value(inner) => write!(f, "Node::Value({:?})", inner),
            Node::Array(inner) => write!(f, "Node::Array({:?})", inner),
            Node::Map(inner) => write!(f, "Node::Map({:?})", inner),
            Node::Scope(inner) => write!(f, "Node::Scope({:?})", inner),
            Node::Empty() => write!(f, "Node::Empty()"),
        }
    }
}

impl Default for Node {
    fn default() -> Self {
        Node::Empty()
    }
}

impl Node {
    pub(crate) fn exec(&self, ctx: &dyn Context) -> Node {
        match self {
            Self::Data(d) => Self::Data(d.clone()),
            Self::Expr(a) => {
                let mut res: Vec<Data> = vec![];
                for node in a.iter() {
                    match node.exec(ctx) {
                        Self::Data(d) => res.push(d),
                        error => return error,
                    };
                }
                if res.is_empty() {
                    Self::Data(Document::Unit.into())
                } else if res.len() == 1 {
                    Self::Data(res.pop().unwrap())
                } else {
                    Self::Data(Data::from_vec(res))
                }
            }
            Self::Value(a) => Self::Data(
                ctx.get_path(&a.iter().map(|a| a).collect::<Vec<&String>>())
                    .into(),
            ),
            Self::Operation(op) => op.exec(ctx),
            Self::Filter(b) => {
                let (piped, filter, args) = (&b.0, &b.1, &b.2);
                let p = piped.exec(ctx).into_document();
                let a = args.exec(ctx).into_document();
                filter(p, a).into()
            }
            Self::Scope(i) => {
                let local_context = ScopedContext::new(ctx);
                i.exec(&local_context)
            }
            Self::Array(s) => {
                let mut elements = vec![];
                for node in s.iter() {
                    match node.exec(ctx) {
                        Self::Data(d) => elements.push(d),
                        error => return error,
                    };
                }
                Self::Data(Data::from_vec(elements))
            }
            Self::Map(m) => {
                let mut map: BTreeMap<Document, Document> = BTreeMap::new();
                for (key, node) in m.iter() {
                    match node.exec(ctx) {
                        Self::Data(d) => map.insert(key.clone(), d.into_doc()),
                        error => return error,
                    };
                }
                Self::Data(map.into())
            }
            Self::Function(m) => {
                let (function, args) = (&m.0, &m.1);
                let a = args.exec(ctx).into_document();
                function(a).into()
            }
            Self::Empty() => Self::Data(Document::Unit.into()),
        }
    }

    pub(crate) fn set_operation(self, op: Operations) -> Node {
        match self {
            Node::Expr(nodes) => Node::Operation(op.build(nodes)),
            _ => self,
        }
    }

    pub(crate) fn into_scope(self) -> Node {
        Node::Scope(Box::new(self))
    }

    pub(crate) fn into_document(self) -> Result<Document> {
        match self {
            Self::Data(d) => Ok(d.result()?.into_doc()),
            _ => Err(TemplarError::RenderFailure(
                "Attempted document conversion on unprocessed node".into(),
            )),
        }
    }

    pub fn render(&self, ctx: &dyn Context) -> Result<String> {
        match self {
            Node::Empty() => Ok("".into()),
            other => Ok(other.exec(ctx).into_document()?.to_string()),
        }
    }
}

impl From<Result<Document>> for Node {
    fn from(doc: Result<Document>) -> Node {
        match doc {
            Ok(d) => Self::Data(d.into()),
            Err(e) => Self::Data(e.into()),
        }
    }
}

impl From<Vec<Node>> for Node {
    fn from(mut n: Vec<Node>) -> Node {
        match n.len() {
            1 => n.pop().unwrap(),
            0 => Node::Empty(),
            _ => Node::Expr(n),
        }
    }
}