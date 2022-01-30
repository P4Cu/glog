use copypasta::{x11_clipboard::X11ClipboardContext, ClipboardProvider};
use log::debug;
use skim::prelude::*;
use vim_key::VimKeyParser;

use crate::{
    app::{App, Entry},
    cmdreactor::{CommandResult, FnCommand},
    input::Input,
    term::Term,
};

macro_rules! AssertArgs {
    ($args:ident, $len:expr) => {
        if $args.len() != $len {
            return Err(format!("Expected {} argument, got {}", $len, $args.len()));
        }
    };
}

// TODO: looks like this should be just an app and app is some abomination currently
pub struct Context<'a> {
    pub app: App<'a>,
    pub input: Input,
    pub clipboard: Option<X11ClipboardContext>,
    pub term: Term,
    pub parser: VimKeyParser<&'a str>,
}

// TODO: help action, most probably we should have struct Actions{}

pub fn actions<'a>() -> Vec<(&'static str, FnCommand<Context<'a>>)> {
    vec![
        ("echo", echo),
        ("quit", quit),
        ("up", up),
        ("down", down),
        ("pageup", page_up),
        ("pagedown", page_down),
        ("top", top),
        ("bottom", bottom),
        ("nodeup", node_up),
        ("nodedown", node_down),
        ("center", node_center),
        ("yank", yank),
        ("select", select),
        ("mode", set_mode),
        ("status", status),
        ("exec", exec),
        ("search", search),
        ("reload", reload),
        ("enter_reload", enter_reload),
    ]
}

impl Context<'_> {
    pub fn render(&mut self) -> Result<(), String> {
        self.term
            .terminal
            .draw(|rect| crate::ui::draw(rect, &mut self.app))
            .map_err(|e| format!("Draw failed with: {e}"))?;
        Ok(())
    }

    fn call_in_shell(&mut self, cmd: String) -> Result<(), std::io::Error> {
        // TODO: add info to help about SHELL
        let shell = std::env::var("SHELL").unwrap_or("bash".into());
        let mut command = std::process::Command::new(shell);
        command
            .current_dir(self.app.repository_path())
            .args(["-c", &cmd]);
        self.term
            .call_external(command)
            .expect("Something went wrong");
        self.term.clear();
        Result::Ok(())
    }
}

pub fn echo(ctx: &mut Context, args: &[&str]) -> CommandResult {
    ctx.app.status = args.join(" ").to_owned();
    Ok(())
}

pub fn reload(ctx: &mut Context, args: &[&str]) -> CommandResult {
    ctx.app
        .reload(Some(args.iter().map(|e| (*e).to_owned()).collect()));
    Ok(())
}

pub fn enter_reload(ctx: &mut Context, args: &[&str]) -> CommandResult {
    AssertArgs!(args, 0);
    let orig_rev_range = std::mem::take(&mut ctx.app.revision_range);
    let revision_range = ["command".to_owned(), "reload".to_owned()]
        .into_iter()
        .chain(orig_rev_range)
        .collect::<Vec<_>>();
    let rv = revision_range.iter().map(|s| s as &str).collect::<Vec<_>>();
    let result = set_mode(ctx, &rv);
    ctx.app.revision_range = revision_range;
    result
}

pub fn quit(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.should_quit = true;
    Ok(())
}

pub fn up(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.prev(1);
    Ok(())
}

pub fn down(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.next(1);
    Ok(())
}

pub fn page_up(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.prev(10);
    Ok(())
}

pub fn page_down(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.next(10);
    Ok(())
}

pub fn top(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.top();
    Ok(())
}

pub fn bottom(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.bottom();
    Ok(())
}

pub fn node_up(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.prev_node();
    Ok(())
}

pub fn node_down(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.next_node();
    Ok(())
}

pub fn node_center(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.center_node();
    Ok(())
}

pub fn yank(ctx: &mut Context, args: &[&str]) -> CommandResult {
    AssertArgs!(args, 1);
    let result = ctx.clipboard
        .as_mut()
        .ok_or_else(|| "No clipboard provider!".to_owned())?
        .set_contents(args[0].to_owned())
        .map_err(|e| format!("Clipboard error: {e}"));
    ctx.app.status = format!("yanked: {}", args[0]);
    result
}

pub fn select(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    ctx.app.select();
    Ok(())
}

pub fn set_mode(ctx: &mut Context, args: &[&str]) -> CommandResult {
    match args.first().copied() {
        Some("command") => {
            let c = args.iter().skip(1).copied().collect::<Vec<_>>().join(" ");
            ctx.app.mode_set(crate::app::Mode::Command(if c.is_empty() {
                None
            } else {
                Some(c)
            }));
            Ok(())
        }
        Some(mode) => Err(format!("Unknown mode {}", mode)),
        _ => Err("Mode parameter is required".to_owned()),
    }
}

pub fn status(ctx: &mut Context, args: &[&str]) -> CommandResult {
    ctx.app.status = args.join(" ");
    Ok(())
}

pub fn exec(ctx: &mut Context, args: &[&str]) -> CommandResult {
    ctx.call_in_shell(shlex::join(args.iter().copied()))
        .map_err(|a| format!("exec failed with: {a}"))
}

struct SearchItem {
    text: String,
    hash: String,
}

impl SkimItem for SearchItem {
    fn text(&self) -> Cow<str> {
        Cow::Borrowed(&self.text)
    }

    fn preview(&self, _context: PreviewContext) -> ItemPreview {
        // TODO: command needs to be executed in repository path
        ItemPreview::Command(format!(
            "git show --color --decorate --abbrev-commit {}",
            self.hash
        ))
    }
}

impl From<Entry> for SearchItem {
    fn from(e: Entry) -> Self {
        let refs = if let Some(r) = &e.git.refs {
            r.heads
                .iter()
                .chain(r.tags.iter())
                .chain(r.remotes.iter())
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            "".to_owned()
        };
        Self {
            text: [
                &e.git.hash[..8],
                e.git.subject.as_str(),
                refs.as_str(),
                "--",
                e.git.author.as_str(),
            ]
            .join(" "),
            hash: e.git.hash,
        }
    }
}

pub fn search(ctx: &mut Context, _args: &[&str]) -> CommandResult {
    let (tx_item, rx_item) = unbounded::<Arc<dyn SkimItem>>();
    ctx.app
        .log
        .iter_all()
        .filter(|e| !e.git.hash.is_empty())
        .map(|e| Into::<SearchItem>::into(e.clone()))
        .for_each(|e| {
            tx_item
                .send(Arc::new(e))
                .expect("Sending element to skim failed")
        });
    drop(tx_item); // so that skim could know when to stop waiting for more items.

    let options = SkimOptionsBuilder::default()
        // .multi(true)
        .preview(Some("")) // preview should be specified to enable preview window
        .no_clear(true)
        .prompt(Some("/"))
        .build()
        .unwrap();

    ctx.term
        .call(|| {
            if let Some(result) = Skim::run_with(&options, Some(rx_item)) {
                match result.final_event {
                    Event::EvActAccept(_) => {
                        // no multi supported so take one
                        if let Some(e) = result.selected_items.first() {
                            let z = (**e).as_any().downcast_ref::<SearchItem>();
                            if let Some(z) = z {
                                ctx.app.goto(z.hash.as_ref());
                            }
                        }
                    }
                    Event::EvActAbort => {}
                    _ => debug!("Not matched event: {:?}", result.final_event),
                };
            }
        })
        .map_err(|e| format!("Error in call: {e}"))?;

    ctx.term.clear();

    Ok(())
}
