use crate::{Account, Iou, Token};

pub trait Envelope {
    type Token: Token;

    fn token(&self) -> Self::Token;

    fn account(&self) -> Account {
        self.token().account()
    }

    fn iou(&self) -> Option<Iou>;
}

pub struct Simple {
    account: Account,
    iou: Iou,
}

impl Envelope for Simple {
    type Token = Account;

    fn token(&self) -> Self::Token {
        todo!()
    }

    fn account(&self) -> Account {
        todo!()
    }
}
