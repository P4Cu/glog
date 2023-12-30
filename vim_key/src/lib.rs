use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

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

fn wrap_modifiers(mut key: String, modifiers: KeyModifiers) -> String {
    if modifiers.contains(KeyModifiers::SHIFT) {
        key = format!("S-{}", key);
    }
    if modifiers.contains(KeyModifiers::CONTROL) {
        key = format!("C-{}", key);
    }
    if modifiers.contains(KeyModifiers::ALT) {
        key = format!("M-{}", key);
    }
    format!("<{}>", key)
}

/// This has to map VIMs <C-v>KEY_PRESS
/// Vim :help key-notation
pub fn to_vim_key(key_event: KeyEvent) -> String {
    use KeyCode as C;
    use KeyModifiers as M;
    macro_rules! key {
        ($k:pat) => {
            KeyEvent { code: $k, .. }
        };
        ($k:pat, $m:pat) => {
            KeyEvent {
                code: $k,
                modifiers: $m,
                ..
            }
        };
    }
    match key_event {
        key!(C::Char(' '), modifiers) => wrap_modifiers("Space".into(), modifiers),
        key!(C::Char(c), M::NONE) => c.to_string(),
        key!(C::Char(c), M::SHIFT) => c.to_string(),
        key!(C::Char(c), mut modifiers) => {
            modifiers.remove(M::SHIFT);
            wrap_modifiers(c.to_string(), modifiers)
        }

        key!(C::F(num), modifiers) => {
            // TODO: check if we really get it
            let mut v = num;
            if modifiers.contains(M::SHIFT | M::CONTROL | M::ALT) {
                // special case
                return format!("<M-C-S-F{}>", num);
            }
            if modifiers.contains(M::SHIFT) {
                v += 12
            }
            if modifiers.contains(M::CONTROL) {
                v += 24
            }
            if modifiers.contains(M::ALT) {
                v += 24
            }
            format!("<F{}>", v)
        }

        key!(C::Backspace, modifiers) => wrap_modifiers("BS".into(), modifiers),
        key!(C::Enter, modifiers) => wrap_modifiers("CR".into(), modifiers),
        key!(C::Left, modifiers) => wrap_modifiers("Left".into(), modifiers),
        key!(C::Right, modifiers) => wrap_modifiers("Right".into(), modifiers),
        key!(C::Up, modifiers) => wrap_modifiers("Up".into(), modifiers),
        key!(C::Down, modifiers) => wrap_modifiers("Down".into(), modifiers),
        key!(C::Home, modifiers) => wrap_modifiers("Home".into(), modifiers),
        key!(C::End, modifiers) => wrap_modifiers("End".into(), modifiers),
        key!(C::PageUp, modifiers) => wrap_modifiers("PageUp".into(), modifiers),
        key!(C::PageDown, modifiers) => wrap_modifiers("PageDown".into(), modifiers),
        key!(C::Tab, modifiers) => wrap_modifiers("Tab".into(), modifiers),
        key!(C::BackTab, modifiers) => wrap_modifiers("BS".into(), modifiers),
        key!(C::Delete, modifiers) => wrap_modifiers("Del".into(), modifiers),
        key!(C::Insert, modifiers) => wrap_modifiers("Insert".into(), modifiers),
        key!(C::Esc, modifiers) => {
            // expection from <c-v><Esc>
            wrap_modifiers("Esc".into(), modifiers)
        }

        key!(C::Null) => todo!("NULL not supported"),
        key!(C::CapsLock) => todo!("CapsLock not supported"),
        key!(C::ScrollLock) => todo!("ScrollLock not supported"),
        key!(C::NumLock) => todo!("NumLock not supported"),
        key!(C::PrintScreen) => todo!("PrintScreen not supported"),
        key!(C::Pause) => todo!("Pause not supported"),
        key!(C::Menu) => todo!("Menu not supported"),
        key!(C::KeypadBegin) => todo!("Keypad not supported"),
        key!(C::Media(_)) => todo!("Media not supported"),
        key!(C::Modifier(_)) => todo!("Single modifier not supported"),
    }
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

impl<T> InnerMap<T> {
    fn flatten_actions(&self, parent_keys: &str) -> Vec<(String, &T)> {
        let mut actions = Vec::new();

        // Process the action of the current InnerMap, if present
        if let Some(ref action) = self.action {
            actions.push((parent_keys.into(), action));
        }

        // Recursively process the InnerMap's map if it exists
        if let Some(ref map) = self.map {
            for (key, inner_map) in map {
                let new_parent_keys = format!("{}{}", parent_keys, to_vim_key(*key));
                actions.extend(inner_map.flatten_actions(&new_parent_keys));
            }
        }

        actions
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

impl<T> VimKeyParser<T>
where
    T: Clone + Display + PartialEq + Debug,
{
    pub fn add_action(&mut self, binding: &str, action: T) -> &mut Self {
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

    pub fn remove_action(/* mut */ self, _binding: &str) {
        todo!("Implement remove_action")
        // let most_inner_map = vim_key(binding).iter().fold(
        //     &mut self.map, |acc, key| {
        //         let map = acc.map.get_or_insert(HashMap::default());
        //         if !map.contains_key(key) {
        //             // TODO: do nothing
        //         }
        //         map.get_mut(key).unwrap()
        // });
        // most_inner_map.action = Some(action);
    }

    pub fn handle_action(&mut self, key: KeyEvent) -> ParsedAction<T> {
        let had_multi_key = !self.multi_key.is_empty();
        self.multi_key.push(key);
        let most_inner_map = self
            .multi_key
            .iter()
            .fold(Some(&self.map), |acc, key| acc?.map.as_ref()?.get(key));
        if let Some(map) = most_inner_map {
            if let Some(action) = &map.action {
                if map.map.is_some() {
                    return ParsedAction::Ambiguous(action.clone());
                } else {
                    self.multi_key.clear();
                    return ParsedAction::Only(action.clone());
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

    pub fn get_actions(&self) -> Vec<(String, &T)> {
        assert_eq!(None, self.map.action, "Action for map root (no key bound)");
        self.map.flatten_actions("".into())
    }

    pub fn get_actions_for_binding(&self, binding: &str) -> Vec<(String, &T)> {
        assert_eq!(None, self.map.action, "Action for map root (no key bound)");
        let x = vim_key(binding).iter().fold(Some(&self.map), |acc, e| {
            if let Some(acc) = acc {
                if let Some(ref map) = acc.map {
                    map.get(e)
                } else {
                    None
                }
            } else {
                None
            }
        });
        if let Some(x) = x {
            x.flatten_actions(binding.into())
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod test {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::{to_vim_key, ParsedAction};

    use super::{vim_key, VimKeyParser};

    macro_rules! key {
        ($k:expr) => {
            KeyEvent::new(KeyCode::Char($k), KeyModifiers::NONE)
        };
        ($k:expr, $m:expr) => {
            KeyEvent::new($k, $m)
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
        let mut parser = VimKeyParser::default();
        parser.add_action("0", 0).add_action("1", 1);
        // .add_action("10", 1) // TODO: add this case
        assert_eq!(ParsedAction::Only(0), parser.handle_action(key!('0')));
        assert_eq!(ParsedAction::Only(1), parser.handle_action(key!('1')));
        assert_eq!(ParsedAction::None, parser.handle_action(key!('2')));
        assert_eq!(ParsedAction::Only(0), parser.handle_action(key!('0')));
    }

    #[test]
    fn test_vim_key_parser_advance_state() {
        let mut parser = VimKeyParser::default();
        parser.add_action("11", 11).add_action("22", 22);
        assert_eq!(ParsedAction::Partial, parser.handle_action(key!('1')));
        assert_eq!(ParsedAction::Partial, parser.handle_action(key!('2')));
        assert_eq!(ParsedAction::Only(22), parser.handle_action(key!('2')));
    }

    #[test]
    fn test_vim_key_parser_clash() {
        let mut parser = VimKeyParser::default();
        parser
            .add_action("0", 0)
            .add_action("1", 1)
            .add_action("10", 10);
        assert_eq!(ParsedAction::Ambiguous(1), parser.handle_action(key!('1')));
        assert_eq!(ParsedAction::Only(10), parser.handle_action(key!('0')));
    }

    #[test]
    fn test_to_vim_key() {
        use KeyCode as K;
        let none = KeyModifiers::NONE;
        let alt = KeyModifiers::ALT;
        let ctrl = KeyModifiers::CONTROL;
        let shift = KeyModifiers::SHIFT;
        assert_eq!(to_vim_key(key!(K::Char('s'), none)), "s");
        assert_eq!(to_vim_key(key!(K::Char('s'), ctrl)), "<C-s>");
        assert_eq!(to_vim_key(key!(K::Char('S'), ctrl | shift)), "<C-S>");
        assert_eq!(to_vim_key(key!(K::Char('S'), shift)), "S");
        assert_eq!(to_vim_key(key!(K::Up, none)), "<Up>");
        assert_eq!(to_vim_key(key!(K::Up, ctrl)), "<C-Up>");
        assert_eq!(to_vim_key(key!(K::Up, ctrl | alt)), "<M-C-Up>");
        assert_eq!(to_vim_key(key!(K::Up, ctrl | alt | shift)), "<M-C-S-Up>");
        assert_eq!(to_vim_key(key!(K::Up, ctrl | shift)), "<C-S-Up>");
    }
}
