use keyvalues_parser::tokens::Token;
use serde::de::{DeserializeSeed, SeqAccess};

use crate::{
    de::Deserializer,
    error::{Error, Result},
};

#[derive(Debug)]
pub struct SeqBuilder<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    maybe_len: Option<usize>,
}

impl<'a, 'de> SeqBuilder<'a, 'de> {
    pub fn new(de: &'a mut Deserializer<'de>) -> Self {
        Self {
            de,
            maybe_len: None,
        }
    }

    pub fn length(mut self, len: usize) -> Self {
        self.maybe_len = Some(len);
        self
    }

    pub fn try_build(self) -> Result<SeqEater<'a, 'de>> {
        match (self.maybe_len, self.de.peek()) {
            (Some(len), Some(Token::SeqBegin)) if len != 1 => {
                Ok(SeqEater::new_set_length(self.de, len))
            }
            // `len` says single element, but `SeqBegin` indicates otherwise
            (Some(_), Some(Token::SeqBegin)) => Err(Error::TrailingTokens),
            (None, Some(Token::SeqBegin)) => Ok(SeqEater::new_variable_length(self.de)),
            // TODO: these can be condensed once 1.53 lands
            (_, Some(Token::ObjBegin)) => Ok(SeqEater::new_single_value(self.de)),
            (_, Some(Token::Str(_))) => Ok(SeqEater::new_single_value(self.de)),
            _ => Err(Error::ExpectedSomeValue),
        }
    }
}

#[derive(Debug)]
pub enum SeqEater<'a, 'de: 'a> {
    SingleValue(SingleValueEater<'a, 'de>),
    SetLength(SetLengthEater<'a, 'de>),
    VariableLength(VariableLengthEater<'a, 'de>),
}

impl<'a, 'de> SeqEater<'a, 'de> {
    pub fn new_single_value(de: &'a mut Deserializer<'de>) -> Self {
        Self::SingleValue(SingleValueEater {
            de,
            finished: false,
        })
    }

    pub fn new_set_length(de: &'a mut Deserializer<'de>, remaining: usize) -> Self {
        // Pop off the marker
        if let Some(Token::SeqBegin) = de.next() {
            Self::SetLength(SetLengthEater { de, remaining })
        } else {
            unreachable!("SeqBegin was peeked");
        }
    }

    pub fn new_variable_length(de: &'a mut Deserializer<'de>) -> Self {
        // Pop off the marker
        if let Some(Token::SeqBegin) = de.next() {
            Self::VariableLength(VariableLengthEater { de })
        } else {
            unreachable!("SeqBegin was peeked");
        }
    }
}

impl<'de, 'a> SeqAccess<'de> for SeqEater<'a, 'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self {
            Self::SingleValue(eater) => eater.next_element_seed(seed),
            Self::SetLength(eater) => eater.next_element_seed(seed),
            Self::VariableLength(eater) => eater.next_element_seed(seed),
        }
    }
}

#[derive(Debug)]
pub struct SingleValueEater<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    finished: bool,
}

impl<'de, 'a> SingleValueEater<'a, 'de> {
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        if self.finished {
            Ok(None)
        } else {
            self.finished = true;
            match self.de.peek() {
                Some(Token::ObjBegin) | Some(Token::Str(_)) => {
                    seed.deserialize(&mut *self.de).map(Some)
                }
                Some(_) => Err(Error::ExpectedSomeValue),
                None => Err(Error::EofWhileParsingSequence),
            }
        }
    }
}

#[derive(Debug)]
pub struct SetLengthEater<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
}

impl<'de, 'a> SetLengthEater<'a, 'de> {
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.de.peek() {
            Some(Token::ObjBegin) | Some(Token::Str(_)) => {
                self.remaining -= 1;
                let val = seed.deserialize(&mut *self.de).map(Some)?;

                // Eagerly pop off the end marker since this won't get called again
                if self.remaining == 0 {
                    match self.de.next() {
                        Some(Token::SeqEnd) => Ok(()),
                        Some(_) => Err(Error::TrailingTokens),
                        None => Err(Error::EofWhileParsingSequence),
                    }?;
                }

                Ok(val)
            }
            Some(Token::SeqEnd) => Err(Error::UnexpectedEndOfSequence),
            Some(_) => Err(Error::ExpectedSomeValue),
            None => Err(Error::EofWhileParsingSequence),
        }
    }
}

#[derive(Debug)]
pub struct VariableLengthEater<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> VariableLengthEater<'a, 'de> {
    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: DeserializeSeed<'de>,
    {
        match self.de.peek() {
            Some(Token::ObjBegin) | Some(Token::Str(_)) => {
                seed.deserialize(&mut *self.de).map(Some)
            }
            Some(Token::SeqEnd) => {
                // Pop off the marker
                if let Some(Token::SeqEnd) = self.de.next() {
                    Ok(None)
                } else {
                    unreachable!("SeqEnd was peeked");
                }
            }
            Some(_) => Err(Error::ExpectedSomeValue),
            None => Err(Error::EofWhileParsingSequence),
        }
    }
}