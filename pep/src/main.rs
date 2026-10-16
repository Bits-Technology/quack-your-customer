//! `pep` — Agent Pep, Duck Diligence Officer, from the terminal.
//!
//! Pep has not reported for duty yet, so every command here drives the **simulated** Pep:
//! `scripts/duck-sim`, the real daemons against a MuJoCo body. This tool adds nothing to that
//! script. It is the short way of saying what the script already does, with the words a person
//! reaches for — `pep do roulade` rather than `scripts/duck-sim ctl robot do roulade`.
//!
//! **A wrapper, on purpose.** The script owns the mechanism: which socket, which params file,
//! which duck, what `down` has to clean up. Teaching this tool any of that would be a second place
//! for it to drift, so each command becomes an argument list for the script and `exec`s it. The
//! process is replaced rather than spawned, which is what lets `pep monitor` own the terminal and
//! `pep log` die on Ctrl-C the way `tail -f` does, with the script's own exit status.
//!
//! Where the checkout is comes from `PEP_REPO` (the flake shell sets it) or, failing that, from
//! where this crate was built — a laptop tool built from the repo it drives. `--dry-run` prints
//! the script invocation instead of running it, which is also how the translation is tested.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// The grouped help, in the shape of `bits` — one screen, by what you are trying to do, rather
/// than clap's alphabetical list. `pep` with nothing after it prints this and exits 0.
const HELP: &str = "\
pep — Agent Pep, Duck Diligence Officer. Bits's smallest compliance hire, from the terminal.

Pep has not reported for duty yet, so every command here drives the SIMULATED Pep:
scripts/duck-sim, the real daemons against a MuJoCo body. pep -n <command> shows the
script invocation instead of running it.

ONBOARDING
  pep wake                  Wake Pep up in the simulator — a MuJoCo window opens, Pep stands
  pep sleep                 Send Pep to sleep: stop the simulator and everything it started
  pep status                Is the loop healthy, and is Pep standing?
  pep health                The full health report (robotctl health)

DUCK DILIGENCE
  pep do <skill>            Perform a trick: roulade, kick_left, kick_right, sit_toggle, ground_pick
  pep skills                The tricks Pep knows, and which policy fills each slot
  pep walk [vx] [vyaw]      Walk for 8 s (default 0.3 m/s straight ahead)
  pep monitor               Live joints, IMU, depth and sticks (q quits)

PAPER TRAIL
  pep log                   What robotd is saying
  pep simlog                What MuJoCo is saying

ESCAPE HATCHES
  pep sim <args…>           Anything scripts/duck-sim does: boot 4, shell, realtime, rootfs
  pep ctl <args…>           Anything robotctl does, aimed at Pep: policy add, pad bind, account login

  pep help [command]        This page, or one command's flags

docs/robot/simulation.md has the simulator; docs/robot/cheatsheet.md has every robotctl command.
";

#[derive(Parser)]
#[command(
    name = "pep",
    version,
    override_help = HELP,
    // `pep`, `pep help` and `pep --help` are the page above; `pep help do` and `pep do --help`
    // are clap's own page for that one verb.
    subcommand_required = false
)]
struct Cli {
    /// Print the script invocation instead of running it.
    #[arg(long, short = 'n', global = true)]
    dry_run: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug, PartialEq)]
enum Command {
    /// Wake Pep up in the simulator: a MuJoCo window opens and Pep stands up.
    Wake,
    /// Send Pep to sleep: stop the simulator and everything it started.
    Sleep,
    /// Is the loop healthy, and is Pep standing?
    Status,
    /// The full health report.
    Health,
    /// Perform a trick by name.
    Do {
        /// `roulade`, `kick_left`, `kick_right`, `sit_toggle`, `ground_pick`, or anything
        /// `pep skills` lists.
        skill: String,
    },
    /// The tricks Pep knows, and which policy fills each slot.
    Skills,
    /// Walk for 8 s.
    Walk {
        /// Forward speed in m/s. The script's default is 0.3, which the twin follows at
        /// about a third; 0.15 steps in place.
        vx: Option<f64>,
        /// Turn rate in rad/s. The script's default is 0.
        vyaw: Option<f64>,
    },
    /// Live joints, IMU, depth and sticks.
    Monitor {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// What robotd is saying.
    Log,
    /// What MuJoCo is saying.
    Simlog,
    /// Anything scripts/duck-sim does.
    Sim {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Anything robotctl does, aimed at Pep.
    Ctl {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// What a command becomes: `scripts/duck-sim`'s arguments. Pure, so the translation is testable
/// without a script to run.
fn plan(command: &Command) -> Vec<String> {
    let strings = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    match command {
        Command::Wake => strings(&["up"]),
        Command::Sleep => strings(&["down"]),
        Command::Status => strings(&["status"]),
        Command::Health => strings(&["ctl", "health"]),
        Command::Do { skill } => strings(&["ctl", "robot", "do", skill]),
        Command::Skills => strings(&["ctl", "policy", "list"]),
        Command::Walk { vx, vyaw } => {
            let mut args = strings(&["drive"]);
            // `drive` is positional — `vx` then `vyaw` — so a turn rate alone still has to say
            // which speed it goes with, and the script's own default is the honest one.
            if let Some(vx) = vx {
                args.push(vx.to_string());
            }
            if let Some(vyaw) = vyaw {
                if vx.is_none() {
                    args.push("0.3".to_owned());
                }
                args.push(vyaw.to_string());
            }
            args
        }
        Command::Monitor { args } => [strings(&["monitor"]), args.clone()].concat(),
        Command::Log => strings(&["log"]),
        Command::Simlog => strings(&["simlog"]),
        Command::Sim { args } => args.clone(),
        Command::Ctl { args } => [strings(&["ctl"]), args.clone()].concat(),
    }
}

/// `PEP_REPO` when set, otherwise the parent of this crate at build time.
fn repo_root(env: Option<&str>) -> PathBuf {
    match env {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the crate lives one level below the workspace root")
            .to_path_buf(),
    }
}

/// The line a person would type to do the same thing by hand, for `--dry-run` and for errors.
fn render(script: &Path, args: &[String]) -> String {
    std::iter::once(script.display().to_string())
        .chain(args.iter().map(|a| {
            if a.is_empty() || a.chars().any(char::is_whitespace) {
                format!("{a:?}")
            } else {
                a.clone()
            }
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        print!("{HELP}");
        return ExitCode::SUCCESS;
    };

    let root = repo_root(std::env::var("PEP_REPO").ok().as_deref());
    let script = root.join("scripts").join("duck-sim");
    let args = plan(&command);

    if cli.dry_run {
        println!("{}", render(&script, &args));
        return ExitCode::SUCCESS;
    }
    if !script.is_file() {
        eprintln!(
            "pep: no {} — set PEP_REPO to the quack-your-customer checkout",
            script.display()
        );
        return ExitCode::FAILURE;
    }
    if matches!(command, Command::Wake) {
        println!("== waking Pep up in the simulator — the real Pep has not reported for duty yet");
    }

    // `exec` returns only when it could not replace the process; a script that ran and failed
    // has already become our exit status.
    let error = {
        use std::os::unix::process::CommandExt;
        std::process::Command::new(&script).args(&args).exec()
    };
    eprintln!("pep: could not run {}: {error}", render(&script, &args));
    ExitCode::FAILURE
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    /// **The whole tool is the translation**, so each verb's spelling is pinned here: the script
    /// is what is tested against a standing duck, and a wrong word on this side is a command
    /// that does something else or nothing.
    #[test]
    fn each_verb_becomes_the_script_command_it_stands_for() {
        assert_eq!(plan(&Command::Wake), args(&["up"]));
        assert_eq!(plan(&Command::Sleep), args(&["down"]));
        assert_eq!(plan(&Command::Status), args(&["status"]));
        assert_eq!(plan(&Command::Health), args(&["ctl", "health"]));
        assert_eq!(
            plan(&Command::Do {
                skill: "roulade".to_owned()
            }),
            args(&["ctl", "robot", "do", "roulade"])
        );
        assert_eq!(plan(&Command::Skills), args(&["ctl", "policy", "list"]));
        assert_eq!(plan(&Command::Log), args(&["log"]));
        assert_eq!(plan(&Command::Simlog), args(&["simlog"]));
    }

    /// **`drive` is positional**, so a turn rate on its own would be read as a speed. The
    /// script's own default speed is put in front of it rather than a number invented here.
    #[test]
    fn walk_keeps_the_scripts_argument_order() {
        assert_eq!(
            plan(&Command::Walk {
                vx: None,
                vyaw: None
            }),
            args(&["drive"])
        );
        assert_eq!(
            plan(&Command::Walk {
                vx: Some(0.2),
                vyaw: None
            }),
            args(&["drive", "0.2"])
        );
        assert_eq!(
            plan(&Command::Walk {
                vx: Some(0.2),
                vyaw: Some(0.5)
            }),
            args(&["drive", "0.2", "0.5"])
        );
        assert_eq!(
            plan(&Command::Walk {
                vx: None,
                vyaw: Some(0.5)
            }),
            args(&["drive", "0.3", "0.5"])
        );
    }

    /// **The escape hatches pass everything through untouched**, flags included: `pep sim boot 2`
    /// and `pep ctl policy add x repo --hold 4` have to reach the script exactly as typed.
    #[test]
    fn the_escape_hatches_forward_their_arguments_verbatim() {
        let cli = Cli::try_parse_from(["pep", "sim", "boot", "2"]).unwrap();
        assert_eq!(plan(cli.command.as_ref().unwrap()), args(&["boot", "2"]));

        let cli = Cli::try_parse_from(["pep", "ctl", "policy", "add", "bow", "u/r", "--hold", "4"])
            .unwrap();
        assert_eq!(
            plan(cli.command.as_ref().unwrap()),
            args(&["ctl", "policy", "add", "bow", "u/r", "--hold", "4"])
        );

        let cli = Cli::try_parse_from(["pep", "-n", "monitor", "--json"]).unwrap();
        assert!(cli.dry_run);
        assert_eq!(
            plan(cli.command.as_ref().unwrap()),
            args(&["monitor", "--json"])
        );
    }

    /// **The help page is written by hand, so it can lie.** Every `pep <verb>` it lists has to
    /// be a verb clap accepts, or the page teaches a command that does not exist.
    #[test]
    fn every_verb_on_the_help_page_exists() {
        let command = Cli::command();
        // `help` is clap's, added when the command is built rather than declared on `Command`.
        let known: Vec<&str> = command
            .get_subcommands()
            .map(|c| c.get_name())
            .chain(["help"])
            .collect();
        let listed: Vec<&str> = HELP
            .lines()
            .filter_map(|line| line.strip_prefix("  pep "))
            .map(|rest| rest.split_whitespace().next().unwrap())
            .collect();
        assert!(!listed.is_empty(), "the page lists its commands");
        for verb in &listed {
            assert!(
                known.contains(verb),
                "help lists `pep {verb}`, which clap does not know"
            );
        }
        for verb in &known {
            assert!(
                listed.contains(verb),
                "`pep {verb}` exists and the help page omits it"
            );
        }
    }

    /// **`PEP_REPO` wins when it is set**, and an empty one does not turn the checkout into `/`.
    #[test]
    fn the_checkout_comes_from_the_environment_first() {
        assert_eq!(repo_root(Some("/somewhere")), PathBuf::from("/somewhere"));
        let built_from = repo_root(None);
        assert!(
            built_from.join("scripts").join("duck-sim").is_file(),
            "{built_from:?}"
        );
        assert_eq!(repo_root(Some("")), built_from);
    }

    /// **`--dry-run` prints a line that can be pasted**, so a value with a space in it is quoted.
    #[test]
    fn the_dry_run_line_is_pasteable() {
        let script = Path::new("scripts/duck-sim");
        assert_eq!(
            render(script, &args(&["ctl", "robot", "do", "roulade"])),
            "scripts/duck-sim ctl robot do roulade"
        );
        assert_eq!(
            render(script, &args(&["ctl", "system", "name", "Agent Pep"])),
            "scripts/duck-sim ctl system name \"Agent Pep\""
        );
    }
}
