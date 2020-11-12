use crate::{
    ir::{BinOp, Literal, UnOp},
    ty::BaseTy,
};

#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Variable(pub String);

#[derive(Debug)]
pub enum Predicate {
    Var(Variable),
    Lit(Literal),
    BinApp(BinOp, Box<Self>, Box<Self>),
    UnApp(UnOp, Box<Self>),
    App(Variable, Vec<Self>),
}

#[derive(Debug)]
pub enum Ty {
    Base(BaseTy),
    RefBase(Variable, BaseTy, Predicate),
    RefFunc(Vec<(Variable, Self)>, Box<Self>),
}

#[derive(Debug)]
pub enum Annotation {
    Ty(Ty),
}
