use std::{collections::HashMap, fmt::Debug};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use pest::{self, iterators::Pair, Parser};

#[derive(pest_derive::Parser, Debug)]
#[grammar = "vim_binding.pest"]
struct VimBindingParser;

pub fn vim_key(binding: &'_ str) -> Vec<KeyEvent> {
    let keys = VimBindingParser::parse(Rule::main, binding).unwrap_or_else(|e| panic!("{}", e));
    keys.flat_map(|pair| {
        pair.into_inner().map(|p| match p.as_rule() {
            Rule::group => p.into_inner().fold(
                KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE),
                |_key, p| match p.as_rule() {
                    Rule::fx_key => parse_fx_key(p),
                    Rule::mod_key => parse_mod_key(p),
                    Rule::space => KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
                    _ => unreachable!(),
                },
            ),
            Rule::key => KeyEvent::new(parse_key(p), KeyModifiers::NONE),
            _ => unreachable!(),
        })
    })
    .collect()
}

/// Parse grammar element fx_key
fn parse_fx_key(p: Pair<Rule>) -> KeyEvent {
    // holds only one inner data: digit
    let inner_pair = p.into_inner().next().unwrap();
    let digit = inner_pair.as_str().parse().unwrap();
    KeyEvent::new(KeyCode::F(digit), KeyModifiers::NONE)
}

/// Parse grammar element mod_key
fn parse_mod_key(p: Pair<Rule>) -> KeyEvent {
    // mod_ctrl = { "c-" | "C-" }
    // mod_alt = { "a-" | "A-" }
    // mod_key = ${ (mod_ctrl|mod_alt) ~ key }
    p.into_inner().fold(
        KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE),
        |mut key, p| {
            match p.as_rule() {
                Rule::mod_ctrl => {
                    key.modifiers = KeyModifiers::CONTROL;
                }
                Rule::mod_alt => {
                    key.modifiers = KeyModifiers::ALT;
                }
                Rule::key => {
                    key.code = parse_key(p);
                }
                _ => unreachable!(),
            }
            key
        },
    )
}

fn parse_key(p: Pair<Rule>) -> KeyCode {
    // TODO: not full implementation
    KeyCode::Char(p.as_str().chars().next().unwrap())
}

#[derive(Debug)]
struct InnerMap<T> {
    action: Option<T>,
    map: Option<HashMap<KeyEvent, InnerMap<T>>>,
}

impl<T> Default for InnerMap<T> {
    fn default() -> InnerMap<T> {
        InnerMap {
            action: None,
            map: None,
        }
    }
}

pub struct VimKeyParser<T> {
    map: InnerMap<T>,
    multi_key: Vec<KeyEvent>,
}

impl<T> Default for VimKeyParser<T> {
    fn default() -> Self {
        Self {
            map: Default::default(),
            multi_key: Vec::default(),
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum ParsedAction<T> {
    Only(T),
    Ambiguous(T),
    Partial,
    None,
}

impl<T: Clone + Copy> VimKeyParser<T> {
    pub fn add_action(mut self, binding: &str, action: T) -> Self {
        let most_inner_map = vim_key(binding).iter().fold(&mut self.map, |acc, key| {
            let map = acc.map.get_or_insert(HashMap::default());
            if !map.contains_key(key) {
                map.insert(*key, InnerMap::default());
            }
            map.get_mut(key).unwrap()
        });
        most_inner_map.action = Some(action);
        self
    }

    pub fn handle_action(&mut self, key: KeyEvent) -> ParsedAction<T> {
        let had_multi_key = !self.multi_key.is_empty();
        self.multi_key.push(key);
        let most_inner_map = self
            .multi_key
            .iter()
            .fold(Some(&self.map), |acc, key| acc?.map.as_ref()?.get(key));
        if let Some(map) = most_inner_map {
            if let Some(action) = map.action {
                if map.map.is_some() {
                    return ParsedAction::Ambiguous(action);
                } else {
                    self.multi_key.clear();
                    return ParsedAction::Only(action);
                }
            } else {
                return ParsedAction::Partial;
            }
        } else {
            self.multi_key.clear();
            if had_multi_key {
                // try once more with clear state
                return self.handle_action(key);
            }
        }
        ParsedAction::None
    }
}

#[cfg(test)]
mod test {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::ParsedAction;

    use super::{vim_key, VimKeyParser};

    macro_rules! key {
        ($k:expr) => {
            KeyEvent::new(KeyCode::Char($k), KeyModifiers::NONE)
        };
    }

    #[test]
    fn simple() {
        assert_eq!(vim_key("a"), vec![key!('a')]);
        assert_eq!(vim_key("abc"), vec![key!('a'), key!('b'), key!('c'),]);
        assert_eq!(vim_key("gg"), vec![key!('g'), key!('g'),]);
        assert_eq!(vim_key("GG"), vec![key!('G'), key!('G'),]);
        assert_eq!(
            vim_key("<f1>"),
            vec![KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)]
        );
        assert_eq!(
            vim_key("<f11>"),
            vec![KeyEvent::new(KeyCode::F(1), KeyModifiers::NONE)]
        );
    }

    #[test]
    fn modifiers() {
        assert_eq!(
            vim_key("<c-c>"),
            vec![KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)]
        );
    }

    #[test]
    fn test_vim_key_parser() {
        let mut parser = VimKeyParser::default()
            .add_action("0", 0)
            .add_action("1", 1);
        // .add_action("10", 1) // TODO: add this case
        assert_eq!(ParsedAction::Only(0), parser.handle_action(key!('0')));
        assert_eq!(ParsedAction::Only(1), parser.handle_action(key!('1')));
        assert_eq!(ParsedAction::None, parser.handle_action(key!('2')));
        assert_eq!(ParsedAction::Only(0), parser.handle_action(key!('0')));
    }

    #[test]
    fn test_vim_key_parser_advance_state() {
        let mut parser = VimKeyParser::default()
            .add_action("11", 11)
            .add_action("22", 22);
        assert_eq!(ParsedAction::Partial, parser.handle_action(key!('1')));
        assert_eq!(ParsedAction::Partial, parser.handle_action(key!('2')));
        assert_eq!(ParsedAction::Only(22), parser.handle_action(key!('2')));
    }

    #[test]
    fn test_vim_key_parser_clash() {
        let mut parser = VimKeyParser::default()
            .add_action("0", 0)
            .add_action("1", 1)
            .add_action("10", 10);
        assert_eq!(ParsedAction::Ambiguous(1), parser.handle_action(key!('1')));
        assert_eq!(ParsedAction::Only(10), parser.handle_action(key!('0')));
    }
}
