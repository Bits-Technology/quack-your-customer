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
  pep do <skill>            Perform a trick: bow, roulade, kick_left, kick_right, sit_toggle, ground_pick
  pep skills                The tricks Pep knows: what `pep do` answers to
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
    ///
    /// Builds the daemons from the branch you are on, fetches the official policy set the first
    /// time, starts the MuJoCo body and the daemons under ~/.cache/duck-sim, and hands Pep to the
    /// policy, which stands it up from a seated start. From then on it is a robot: the same
    /// robotd, the same robotctl, the same 50 Hz loop, with the body in MuJoCo.
    ///
    /// Environment knobs, all optional (docs/robot/simulation.md has the full table):
    ///   DUCK_SIM_VIEWER=0          no window — cheaper, and the fix when the world runs below 1.0x
    ///   DUCK_SIM_SCENE=apartment   six rooms instead of a bare floor
    ///   DUCK_SIM_CAMERAS=a         give Pep an eye; the console is then at http://127.0.0.1:8080
    ///   DUCK_SIM_KEYFRAME=STAND    start standing instead of seated
    ///   DUCK_SIM_VIEW=follow       the camera follows Pep (default); free is the mouse camera;
    ///                              follow:0.5,90,-30 is metres, azimuth, elevation
    ///
    /// In the window, no mouse needed: arrows orbit and tilt, Page Up/Down zoom, Home follows
    /// Pep, End is the free overview, and each resets its view when pressed again. The letters
    /// belong to the viewer: F toggles contact-force arrows, C contact points, T transparency.
    ///
    /// Needs a microduck_rl checkout with its venv beside this repo, or wherever DUCK_SIM_RL
    /// points. For the real unit files under systemd, `pep sim boot 4` instead.
    #[command(verbatim_doc_comment)]
    Wake,
    /// Send Pep to sleep: stop the simulator and everything it started.
    ///
    /// Stops the body and every daemon `pep wake` started, by pidfile, and any containers
    /// `pep sim boot` left running. Nothing to confirm, and the MuJoCo window closes with it.
    /// Nothing falls over on the way out: the body is a simulation, and it is simply gone until
    /// the next wake.
    Sleep,
    /// Is the loop healthy, and is Pep standing?
    ///
    /// The first lines of `robotctl health` — the 50 Hz loop and how many ticks it missed, the
    /// bus, the IMU — then the simulator's own verdict from the trunk: standing is about
    /// 0.12 m, seated about 0.07, and gravity in the trunk frame says upright or on its back.
    /// A loop below 45 Hz is unhealthy rather than slow; the usual cause is the world running
    /// below real time (`pep sim realtime`), and the usual fix is DUCK_SIM_VIEWER=0.
    Status,
    /// The full health report.
    ///
    /// `robotctl health` in full: the loop, the bus, the IMU, the policy that is driving and
    /// whether it loaded, battery and thermals (faked here), and when the update source last
    /// answered. `pep status` is the short form.
    Health,
    /// Perform a trick by name.
    ///
    /// Asks the robot to run one skill, once, and returns at once: "queued" means the loop took
    /// it and starts it on the next tick. Pep has to be driving for a skill to run, which it is
    /// after `pep wake`.
    ///
    /// The tricks every duck ships with:
    ///   bow            a polite bow: lean forward, head down, hold, and back up — about 2 s
    ///   roulade        a forward roll over the head, 1 s, landing back on its feet
    ///   kick_left      a left-leg kick, 0.5 s
    ///   kick_right     a right-leg kick, 0.5 s
    ///   sit_toggle     sit down, or stand back up if seated
    ///   ground_pick    crouch, touch the ground with the beak, stand back up
    ///
    /// `pep skills` lists what this Pep has, including anything added from the Hub with
    /// `pep ctl policy add <name> <user>/microduck-<name> [--hold <seconds>]`. A name Pep does
    /// not know is refused with the list it does. `sitstand` is accepted for `sit_toggle` — it
    /// is the gait slot's name, not the trick's — and `kick-left` for `kick_left`.
    #[command(verbatim_doc_comment)]
    Do {
        /// The skill's name, as `pep skills` lists it. Hyphens are fine; `sitstand` means
        /// `sit_toggle`.
        #[arg(value_name = "skill")]
        skill: String,
    },
    /// The tricks Pep knows.
    ///
    /// The skills table from `robotctl policy list`: what `pep do` answers to, how long each
    /// runs, and the policy behind it; two are driven by the robot itself and have no file of
    /// their own. The listing's other table — the gait slots, walk and stand and sitstand and
    /// the rest, each with where its policy came from — is under `pep ctl policy list`. Slot
    /// names look like tricks and are not: `sitstand` is the slot, `sit_toggle` the trick.
    Skills,
    /// Walk for 8 s.
    ///
    /// Sends the velocity intent ten times a second for eight seconds, then stops sending: the
    /// robot's deadman zeroes the twist half a second after the last one, so Pep stops on its
    /// own and nothing is left running. Both numbers are positional, so a turn rate on its own
    /// is given the default speed in front of it.
    ///
    /// In the twin the gait follows about a third of the speed it is asked for: the default of
    /// 0.3 m/s is about 0.75 m over the eight seconds, and 0.15 steps in place. The pad drives
    /// a robot at up to 0.4 m/s and 1.0 rad/s, the range the gait was trained on — asking for
    /// more is asking, not getting.
    Walk {
        /// Forward speed, m/s. Negative walks backwards.
        ///
        /// The script's default is 0.3. The twin follows about a third of it, 0.15 steps in
        /// place, and the gait is trained to 0.4.
        #[arg(value_name = "vx", allow_negative_numbers = true)]
        vx: Option<f64>,
        /// Turn rate, rad/s. Positive turns left.
        ///
        /// Default 0; the gait is trained to 1.0. Given without a speed, the default speed goes
        /// in front of it, because the script reads the two by position.
        #[arg(value_name = "vyaw", allow_negative_numbers = true)]
        vyaw: Option<f64>,
    },
    /// Live joints, IMU, depth and sticks.
    ///
    /// `robotctl monitor` aimed at Pep: one frame repainted in place — what a client asked for
    /// beside what safety actually applied, every joint measured against commanded, the depth
    /// matrix, the pad's sticks, and the policy that is driving. `q` quits, ↑/↓ scroll the
    /// joints. Piped or redirected it prints a line per tick instead; `--json` carries the whole
    /// state, and `--hz <n>` asks the robot for fewer frames.
    Monitor {
        /// robotctl monitor's own flags: --hz <n>, --json.
        #[arg(
            value_name = "args",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    /// What robotd is saying.
    ///
    /// `tail -f` on the daemon's log under ~/.cache/duck-sim: policy swaps, skills starting,
    /// the fall verdict, the deadman tripping, and the once-a-minute loop statistics. Ctrl-C
    /// stops it.
    Log,
    /// What MuJoCo is saying.
    ///
    /// `tail -f` on the body's log: daemons connecting, a viewer that could not open, and
    /// "behind real time", which is the line that explains a duck that cannot balance. Ctrl-C
    /// stops it.
    Simlog,
    /// Anything scripts/duck-sim does.
    ///
    /// The script's own commands, passed through as typed:
    ///   boot [N]       N ducks (default 1), each under real systemd in its own container (sudo)
    ///   shell [duck]   a shell on duck-a, or the one named — containers only
    ///   stand          hand Pep to its policy and check that it stood
    ///   realtime       how fast the world is running; below 1.0x the policies cannot balance
    ///   rootfs         rebuild the Debian rootfs the containers boot from
    ///   up · down · drive · ctl · status · monitor · log · simlog   what the pep verbs wrap
    ///
    /// Several ducks share one world and can bump into each other; DUCK_SIM_DUCK=duck-b aims
    /// the other verbs at a different one. docs/robot/simulation.md is the full page.
    #[command(verbatim_doc_comment)]
    Sim {
        /// The script's command and its arguments, verbatim.
        #[arg(
            value_name = "args",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
        args: Vec<String>,
    },
    /// Anything robotctl does, aimed at Pep.
    ///
    /// `robotctl` with Pep's sockets and params file filled in, so every command in
    /// docs/robot/cheatsheet.md works here as on a robot. The ones worth knowing:
    ///   policy list · search <q> · add <name> <repo> [--hold s] · load <slot> <x> · reset [slot]
    ///   pad bindings · bind <button> <skill> · reset [button]
    ///   robot do <skill> · mode walk|roller · look <x> <y> <z> · relax · init
    ///   account login · status · logout       reach Pep from anywhere, like a real duck
    ///   system info                           serial, name, and the line "body MuJoCo"
    ///   configure                             every params key, with its current value
    ///   quack · chorale · theremin
    /// Commands that record a change — policy add, policy load, pad bind — write the
    /// simulator's params file, so there is no sudo here.
    #[command(verbatim_doc_comment)]
    Ctl {
        /// robotctl's command and its arguments, verbatim.
        #[arg(
            value_name = "args",
            trailing_var_arg = true,
            allow_hyphen_values = true
        )]
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
        Command::Do { skill } => strings(&["ctl", "robot", "do", &skill_name(skill)]),
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

/// The name the robot answers to, from the one a person typed. `sitstand` is the *slot* that
/// drives sitting and standing and the trick is `sit_toggle`; the listing shows both and the
/// first was typed within an hour of the listing existing. Hyphens become underscores for the
/// same reason: `kick-left` reads naturally and the robot spells it `kick_left`.
fn skill_name(typed: &str) -> String {
    let name = typed.replace('-', "_");
    match name.as_str() {
        "sitstand" | "sit_stand" => "sit_toggle".to_owned(),
        _ => name,
    }
}

/// `pep skills` is the tricks: the second of `robotctl policy list`'s two tables, from its
/// `SKILL` header on. The first table is the gait slots, whose names look like tricks and are
/// not — `sitstand` there, `sit_toggle` here — and listing both under "skills" is what sent the
/// slot name to `pep do`. Anything unexpected is printed whole: a listing is worth more than a
/// filter that outgrew it.
fn tricks_only(listing: &str) -> String {
    let Some(at) = listing.find("SKILL") else {
        return listing.to_owned();
    };
    let from = listing[..at].rfind('\n').map_or(0, |i| i + 1);
    format!(
        "{}\n`pep do <skill>` runs one. The gait slots are under `pep ctl policy list`.\n",
        listing[from..].trim_end()
    )
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

/// `pep walk help` is what people type, and clap would read `help` as a speed. Spelled the way
/// clap wants it before parsing — except for the escape hatches, whose `help` belongs to the
/// program they forward to.
fn spell_help(args: Vec<String>) -> Vec<String> {
    match args.as_slice() {
        [program, verb, word] if word == "help" && verb != "sim" && verb != "ctl" => {
            vec![program.clone(), "help".to_owned(), verb.clone()]
        }
        _ => args,
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse_from(spell_help(std::env::args().collect()));
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

    if matches!(command, Command::Skills) {
        return match std::process::Command::new(&script).args(&args).output() {
            Ok(out) => {
                print!("{}", tricks_only(&String::from_utf8_lossy(&out.stdout)));
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
                if out.status.success() {
                    ExitCode::SUCCESS
                } else {
                    ExitCode::FAILURE
                }
            }
            Err(error) => {
                eprintln!("pep: could not run {}: {error}", render(&script, &args));
                ExitCode::FAILURE
            }
        };
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

    /// **The slot's name is not the trick's name**, and the listing shows both. `sitstand` was
    /// typed within an hour of `pep skills` existing; the robot refused it, correctly, with a
    /// list the person had just read.
    #[test]
    fn do_accepts_the_spellings_people_type() {
        assert_eq!(skill_name("sitstand"), "sit_toggle");
        assert_eq!(skill_name("sit-stand"), "sit_toggle");
        assert_eq!(skill_name("kick-left"), "kick_left");
        assert_eq!(skill_name("roulade"), "roulade");
        assert_eq!(skill_name("polite-bow"), "polite_bow");
        assert_eq!(
            plan(&Command::Do {
                skill: "sitstand".to_owned()
            }),
            args(&["ctl", "robot", "do", "sit_toggle"])
        );
    }

    /// **`pep skills` is the tricks table and nothing above it**, and a listing without the
    /// header it looks for is printed whole rather than swallowed.
    #[test]
    fn skills_keeps_the_tricks_and_drops_the_slots() {
        let listing = "mode: walk\n\n   SLOT   ORIGIN  POLICY\n * walk   local   velstand.onnx\n\n\
                       * 6 slots set by config.\n\n   SKILL     RUNS FOR  POLICY\n   roulade   1 s  r.onnx\n\
                       sit_toggle  —  driven by the robot itself\n";
        let shown = tricks_only(listing);
        assert!(shown.starts_with("   SKILL"), "{shown}");
        assert!(shown.contains("sit_toggle"), "{shown}");
        assert!(!shown.contains("SLOT"), "{shown}");
        assert!(shown.contains("pep ctl policy list"), "{shown}");

        let other = "something else entirely\n";
        assert_eq!(tricks_only(other), other);
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

    /// **Every verb has a page, not only a one-liner.** `pep help walk` has to say what `vx` and
    /// `vyaw` are, and a verb added with a single doc line would print nothing past its name.
    #[test]
    fn every_verb_explains_itself() {
        for sub in Cli::command().get_subcommands() {
            assert!(
                sub.get_long_about().is_some(),
                "`pep {}` has a one-liner and no page",
                sub.get_name()
            );
            for arg in sub.get_arguments().filter(|a| a.is_positional()) {
                assert!(
                    arg.get_help().is_some(),
                    "`pep {} {}` has no description",
                    sub.get_name(),
                    arg.get_id()
                );
            }
        }
    }

    /// **`pep walk help` means `pep help walk`**, and `pep ctl help` still means robotctl's
    /// own help, because that word belongs to the program `ctl` forwards to.
    #[test]
    fn help_after_a_verb_is_help_for_the_verb() {
        let words = |items: &[&str]| items.iter().map(|w| (*w).to_owned()).collect::<Vec<_>>();
        assert_eq!(
            spell_help(words(&["pep", "walk", "help"])),
            words(&["pep", "help", "walk"])
        );
        assert_eq!(
            spell_help(words(&["pep", "ctl", "help"])),
            words(&["pep", "ctl", "help"])
        );
        assert_eq!(
            spell_help(words(&["pep", "walk", "0.2"])),
            words(&["pep", "walk", "0.2"])
        );
    }

    /// **A negative speed walks backwards**, which clap would otherwise read as a flag.
    #[test]
    fn walk_accepts_negative_numbers() {
        let cli = Cli::try_parse_from(["pep", "walk", "-0.2", "-0.5"]).unwrap();
        assert_eq!(
            cli.command,
            Some(Command::Walk {
                vx: Some(-0.2),
                vyaw: Some(-0.5)
            })
        );
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
