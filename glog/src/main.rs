mod actions;
mod app;
mod cmdreactor;
mod input;
mod stateful_list;
mod term;
mod ui;
mod utils;

use app::App;
use cmdreactor::CommandResult;
use input::InputEvent;
use log::trace;
use std::error::Error;
use tokio::select;
use tui_textarea::{Input, Key};

use clap::Parser;

use vim_key::{ParsedAction, VimKeyParser};

use crate::{cmdreactor::CmdReactor, term::Term};

// TODO: support: --all / --since / --before
// TODO: do not allow to specify non revision
// TODO: https://stackoverflow.com/questions/17639383/how-to-add-missing-origin-head-in-git-repo

/// git-log on steroids
#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// git repository path
    #[clap(short, long)]
    repository: Option<String>,
    /// as specified in git-log command eg. HEAD "^HEAD~5"
    revision_range: Vec<String>,
}

#[allow(clippy::single_match)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    if let Err(err) = syslog::init_unix(syslog::Facility::LOG_USER, log::LevelFilter::Trace) {
        eprintln!("Could not init syslog: {}", err);
    } else {
        log_panics::init();
    }

    let cli = Cli::parse();
    let repository = std::fs::canonicalize(cli.repository.unwrap_or_else(|| "./".to_string()))?;

    // TODO: bind via config file
    // TODO: <cr> executes commands, othewise enter pre-filled command mode
    // TODO: allow shorter commands when not conflicting
    // TODO: allow way to bind new commands
    let parser = VimKeyParser::default()
        .add_action("q", "quit")
        .add_action("<c-c>", "quit")
        .add_action("k", "up")
        .add_action("j", "down")
        .add_action("<c-u>", "pageup")
        .add_action("<c-d>", "pagedown")
        .add_action("gg", "top")
        .add_action("G", "bottom")
        .add_action("K", "nodeup")
        .add_action("J", "nodedown")
        .add_action("L", "exec git show --stat --patch %0")
        .add_action("yy", "yank %0")
        // TODO: something like %0:branch[@] which would return branch name
        .add_action("zz", "center")
        .add_action("<space>", "select")
        .add_action("d", "exec git diff %_1 %0 ")
        .add_action("D", "exec git difftool --dir-diff %_1 %0")
        // .add_action("@", "exec %@") // TODO: this should enter command without triggering it
        .add_action("/", "search")
        .add_action(":", "mode command")
        .add_action("r", "enter_reload");

    let mut cmd_reactor = CmdReactor::new();
    cmd_reactor.add_commands(actions::actions());

    let context = actions::Context {
        app: App::new(repository, cli.revision_range),
        clipboard: copypasta::ClipboardContext::new().ok(),
        input: input::Input::new(),
        term: Term::new()?,
        parser,
    };

    mainloop(context, cmd_reactor).await
}

async fn mainloop<'a>(
    mut context: actions::Context<'a>,
    mut cmd_reactor: CmdReactor<actions::Context<'a>>,
) -> Result<(), Box<dyn Error>> {
    context.app.reload(None);

    while !context.app.should_quit {
        trace!("loop");

        // TODO: rendering should not happen each frame, more like with delay of 30ms so more frames are grouped together
        context.render()?;

        select! {
            _ = context.app.process() => {},
            event = context.input.next() => {
                handle_input_event(event, &mut context, &mut cmd_reactor);
            },
        }
    }
    Ok(())
}

fn execute<'a>(
    cmd_reactor: &mut CmdReactor<actions::Context<'a>>,
    ctx: &mut actions::Context<'a>,
    line: &str,
) {
    let mut inner_fn = || -> CommandResult {
        // pre-process
        let line = if let Some(stripped) = line.strip_prefix('!') {
            format!("exec {}", stripped)
        } else {
            line.to_owned()
        };

        let words = shlex::split(&line).ok_or_else(|| "Failed to parse command line".to_owned())?;
        let name = words
            .first()
            .ok_or_else(|| "There's no name in command line".to_owned())?;

        let args: Result<Vec<_>, _> = words
            .iter()
            .skip(1)
            .filter_map(|a| -> Option<Result<String, String>> {
                match a.as_str() {
                    "%0" => {
                        let v = ctx
                            .app
                            .current_sha() //asdf
                            .ok_or_else(|| "No sha".to_owned());
                        Some(v)
                    }
                    "%_1" => {
                        // if there's no selected0 this will be None and will be filtered
                        ctx.app.log.selected0().map(|e| Ok(e.git.hash.clone()))
                    }
                    "%1" => {
                        let v = ctx
                            .app
                            .log
                            .selected0()
                            .map(|e| e.git.hash.clone())
                            .ok_or_else(|| "No selection".to_owned());
                        Some(v)
                    }
                    "%%" => Some(Ok("%".to_owned())),
                    _ => Some(Ok(a.to_owned())),
                }
            })
            .collect();
        let args = args?;

        cmd_reactor.execute(ctx, name, args)
    };

    match inner_fn() {
        // TODO: we need a nicer way to handle status so we don't always erase previous (maybe
        // count repeated messages so it's visiable that You press the same key over and over?)
        Ok(..) => {} // ctx.app.status.clear(),
        Err(e) => ctx.app.status = e,
    };
}

fn handle_input_event<'a>(
    event: InputEvent,
    context: &mut actions::Context<'a>,
    cmd_reactor: &mut CmdReactor<actions::Context<'a>>,
) {
    match event {
        input::InputEvent::Event(crossterm::event::Event::Key(e)) => match context.app.mode() {
            app::Mode::Normal => match context.parser.handle_action(e) {
                ParsedAction::Only(action) => {
                    execute(cmd_reactor, context, action);
                }
                ParsedAction::None => {
                    context.app.status = format!("Not handled: {:?}", e);
                }
                ParsedAction::Ambiguous(_) => {}
                ParsedAction::Partial => {}
            },
            app::Mode::Command(_cmd) => {
                let textarea = &mut context.app.textarea;
                match e.into() {
                    Input { key: Key::Esc, .. } => context.app.mode_set(app::Mode::Normal),
                    Input {
                        key: Key::Enter, ..
                    }
                    | Input {
                        key: Key::Char('m'),
                        ctrl: true,
                        ..
                    } => {
                        let cmd = textarea.lines().last().expect("Command cannot be empty");
                        let cmd = cmd[1..].to_owned();
                        context.app.status = format!("Command: {}", cmd);
                        context.app.mode_set(app::Mode::Normal);

                        execute(cmd_reactor, context, cmd.as_str());
                    }
                    input => {
                        if textarea.input(input)
                            && textarea
                                .lines()
                                .last()
                                .expect("there's always one line")
                                .is_empty()
                        {
                            context.app.status = "Command mode quit".to_owned();
                            context.app.mode_set(app::Mode::Normal);
                        }
                    }
                }
            }
        },
        _ => {}
    }
}
