use super::command::Command;
use super::complete::{
    completes_labels, COMPLETE_SUBCOMMAND, DYNAMIC_MARKER, HOOK_TRIGGERS, HOOK_VERBS,
};
use super::grammar::Cli;
use super::ArgsError;

pub const COMPLETION_SHELLS: &[&str] = &["bash", "zsh", "fish", "powershell"];

pub const POWERSHELL_PIPELINE_ANCHOR: &str = "    $completions.Where{";

fn sorted_join(names: &[&str]) -> String {
    let mut sorted: Vec<&str> = names.to_vec();
    sorted.sort_unstable();
    sorted.join(" ")
}

fn update_set_names() -> Vec<&'static str> {
    dx_update::sets::SetId::ALL
        .iter()
        .map(|id| id.name())
        .collect()
}

fn bump_set_names() -> Vec<&'static str> {
    dx_bump::BumpSet::names()
}

fn bash_dynamic(text: &mut String) -> Result<(), ArgsError> {
    let anchor = "                *)\n                    COMPREPLY=()\n                    ;;";
    let dynamic = format!(
        "                *)\n                    #{DYNAMIC_MARKER}: completion callback.\n                    _dx_words=(\"${{COMP_WORDS[@]:1:COMP_CWORD}}\")\n                    if [[ ${{cur}} == \"\" ]]; then\n                        _dx_words+=(\"\")\n                    fi\n                    if command -v dx >/dev/null 2>&1; then\n                        COMPREPLY=( $(dx {COMPLETE_SUBCOMMAND} \"${{_dx_words[@]}}\" 2>/dev/null) )\n                    else\n                        COMPREPLY=()\n                    fi\n                    return 0\n                    ;;"
    );
    if text.contains(anchor) {
        *text = text.replacen(anchor, &dynamic, 1);
        Ok(())
    } else {
        Err(ArgsError::UnknownShell {
            shell: "bash".to_owned(),
        })
    }
}

fn zsh_dynamic(text: &mut String) -> Result<(), ArgsError> {
    let anchor = "*::targets -- Scopes to run on:_default";
    let replacement = "*::targets -- Scopes to run on:_dx_dynamic_targets";
    if !text.contains(anchor) {
        return Err(ArgsError::UnknownShell {
            shell: "zsh".to_owned(),
        });
    }
    *text = text.replacen(anchor, replacement, 1);
    text.push_str(&format!(
        "\n#{DYNAMIC_MARKER}: completion callback.\n(( $+functions[_dx_dynamic_targets] )) ||\n_dx_dynamic_targets() {{\n    local -a _dx_words _dx_candidates\n    _dx_words=(${{words[2,-1]}})\n    if (( CURRENT > $#words )); then\n        _dx_words+=(\"\")\n    fi\n    _dx_candidates=(\"${{(@f)$(dx {COMPLETE_SUBCOMMAND} \"${{_dx_words[@]}}\" 2>/dev/null)}}\")\n    compadd -a _dx_candidates\n}}\n"
    ));
    Ok(())
}

fn fish_dynamic(text: &mut String) {
    use clap::ValueEnum;
    text.push_str(&format!("\n#{DYNAMIC_MARKER}: completion callback.\n"));
    let mut line = |condition: &str, names: &[&str], desc: &str| {
        text.push_str(&format!(
            "complete -c dx -f -n '{condition}' -a '{}' -d '{desc}'\n",
            sorted_join(names)
        ));
    };
    line(
        "__fish_seen_subcommand_from watch",
        dx_adopt::WATCHABLE_COMMANDS,
        "watchable task",
    );
    line(
        "__fish_seen_subcommand_from hooks; and not __fish_seen_subcommand_from install uninstall status run",
        HOOK_VERBS,
        "hooks verb",
    );
    line(
        "__fish_seen_subcommand_from hooks; and __fish_seen_subcommand_from run",
        HOOK_TRIGGERS,
        "hook trigger",
    );
    line(
        "__fish_seen_subcommand_from new",
        dx_adopt::SUPPORTED_NEW_LANGUAGES,
        "project language",
    );
    line(
        "__fish_seen_subcommand_from completion",
        COMPLETION_SHELLS,
        "completion shell",
    );
    line(
        "__fish_seen_subcommand_from update",
        &update_set_names(),
        "dependency set",
    );
    line(
        "__fish_seen_subcommand_from bump",
        &bump_set_names(),
        "dependency set",
    );
    let mut label_commands: Vec<&str> = Command::value_variants()
        .iter()
        .filter(|command| completes_labels(**command))
        .map(|command| command.name())
        .collect();
    label_commands.sort_unstable();
    text.push_str(&format!(
        "complete -c dx -n '__fish_seen_subcommand_from {}' -a '(if test -z (commandline -ct); dx {COMPLETE_SUBCOMMAND} (commandline -opc)[2..-1] \"\"; else; dx {COMPLETE_SUBCOMMAND} (commandline -opc)[2..-1]; end 2>/dev/null)'\n",
        label_commands.join(" ")
    ));
}

fn powershell_dynamic(text: &mut String) -> Result<(), ArgsError> {
    let block = format!(
        "    if ($command -ne 'dx') {{\n        #{DYNAMIC_MARKER}: completion callback.\n        try {{\n            $dxWords = @()\n            for ($i = 1; $i -lt $commandElements.Count; $i++) {{\n                $element = $commandElements[$i]\n                if ($element -is [StringConstantExpressionAst] -and $element.Value -ne $wordToComplete) {{\n                    $dxWords += $element.Value\n                }}\n            }}\n            $dxDynamic = @(dx {COMPLETE_SUBCOMMAND} @dxWords \"$wordToComplete\" 2>$null)\n            foreach ($candidate in $dxDynamic) {{\n                if ($candidate -ne '') {{\n                    $completions += [CompletionResult]::new($candidate, $candidate, [CompletionResultType]::ParameterValue, $candidate)\n                }}\n            }}\n        }} catch {{}}\n    }}\n"
    );
    match text.find(POWERSHELL_PIPELINE_ANCHOR) {
        Some(pos) => {
            text.insert_str(pos, &block);
            Ok(())
        }
        None => Err(ArgsError::UnknownShell {
            shell: "powershell".to_owned(),
        }),
    }
}

pub fn render_completion(shell: &str) -> Result<String, ArgsError> {
    use clap::CommandFactory;
    if !COMPLETION_SHELLS.contains(&shell) {
        return Err(ArgsError::UnknownShell {
            shell: shell.to_owned(),
        });
    }
    let generator =
        shell
            .parse::<clap_complete::aot::Shell>()
            .map_err(|_| ArgsError::UnknownShell {
                shell: shell.to_owned(),
            })?;
    let mut command = Cli::command();
    let mut script = Vec::new();
    clap_complete::generate(generator, &mut command, "dx", &mut script);
    let mut text = String::from_utf8(script).map_err(|_| ArgsError::UnknownShell {
        shell: shell.to_owned(),
    })?;
    match shell {
        "fish" => {
            use clap::ValueEnum;
            text.push_str("\n# dx commands.\n");
            for cmd in Command::value_variants() {
                let desc = cmd.describe().replace('\'', "\\'");
                text.push_str(&format!(
                    "complete -c dx -f -n '__fish_use_subcommand' -a {} -d '{}'\n",
                    cmd.name(),
                    desc
                ));
            }
            fish_dynamic(&mut text);
        }
        "powershell" => {
            use clap::ValueEnum;
            let mut additions = String::new();
            for cmd in Command::value_variants() {
                let desc = cmd.describe().replace('\'', "''");
                additions.push_str(&format!(
                    "            [CompletionResult]::new('{}', '{}', [CompletionResultType]::ParameterValue, '{}')\n",
                    cmd.name(),
                    cmd.name(),
                    desc
                ));
            }
            let anchor = "            break\n        }\n    })";
            if let Some(pos) = text.find(anchor) {
                text.insert_str(pos, &additions);
            } else {
                return Err(ArgsError::UnknownShell {
                    shell: shell.to_owned(),
                });
            }
            powershell_dynamic(&mut text)?;
        }
        "bash" => {
            bash_dynamic(&mut text)?;
        }
        "zsh" => {
            zsh_dynamic(&mut text)?;
        }
        _ => {}
    }
    Ok(text)
}
