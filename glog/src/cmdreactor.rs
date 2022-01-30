use std::collections::HashMap;

use log::{debug, error};

pub type CommandResult = Result<(), String>;
pub type FnCommand<CONTEXT> = fn(&mut CONTEXT, &[&str]) -> CommandResult;

pub struct CmdReactor<T> {
    commands: HashMap<&'static str, FnCommand<T>>,
}

impl<T> CmdReactor<T> {
    pub fn new() -> Self {
        CmdReactor {
            commands: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn add_command(&mut self, name: &'static str, cmd: FnCommand<T>) {
        match self.commands.entry(name) {
            std::collections::hash_map::Entry::Occupied(_) => {
                error!("Already contains command: {}", name)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(cmd);
            }
        }
    }

    pub fn add_commands<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = (&'static str, FnCommand<T>)>,
    {
        // TODO: warning when overriding command
        self.commands.extend(iter);
    }

    pub fn execute(&self, context: &mut T, command: &'_ str, args: Vec<String>) -> CommandResult {
        debug!("Executing: {} with args {:?}", command, args);

        let cmd = self
            .commands
            .get(command)
            .ok_or(format!("Command not found: {command}"))?;
        let z: &Vec<&str> = &args.iter().map(|s| s as &str).collect();
        cmd(context, z)
    }
}

#[cfg(test)]
mod test {
    use super::CmdReactor;

    struct Context<'a> {
        pub number: &'a mut i32,
    }

    #[test]
    fn simple() {
        let mut x = 100;

        let mut reactor = CmdReactor::<Context>::new();
        reactor.add_command("add", |ctx, _args| {
            *ctx.number += 100;
            Result::Ok(())
        });

        let mut d = Context { number: &mut x };
        {
            assert!(reactor.execute(&mut d, "add", vec![]).is_ok());
            assert!(reactor.execute(&mut d, "add", vec![]).is_ok());
        }
        assert_eq!(x, 300);
    }
}
