use crate::Account;

pub trait Token {
    fn account(&self) -> Account;
}

impl Token for Account {
    fn account(&self) -> Account {
        self.clone()
    }
}
