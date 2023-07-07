use std::{thread, time::Duration};

use sudo_test::{Command, Env, TextFile};

use crate::{Result, SUDOERS_ALL_ALL_NOPASSWD};

mod flag_check;
mod flag_help;
mod flag_quiet;
mod flag_version;
mod what_now_prompt;

const ETC_SUDOERS: &str = "/etc/sudoers";
const DEFAULT_EDITOR: &str = "/usr/bin/editor";
const LOGS_PATH: &str = "/tmp/logs.txt";
const CHMOD_EXEC: &str = "100";
const EDITOR_TRUE: &str = "#!/bin/sh
true";

#[test]
fn default_editor_is_usr_bin_editor() -> Result<()> {
    let expected = "default editor was called";
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                "#!/bin/sh

echo '{expected}' > {LOGS_PATH}"
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let actual = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn creates_sudoers_file_with_default_ownership_and_perms_if_it_doesnt_exist() -> Result<()> {
    let env = Env("")
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)?
        .assert_success()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let ls_output = Command::new("ls")
        .args(["-l", ETC_SUDOERS])
        .output(&env)?
        .stdout()?;

    assert!(ls_output.starts_with("-r--r----- 1 root root"));

    Ok(())
}

#[test]
fn errors_if_currently_being_edited() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
sleep 3",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let child = Command::new("visudo").spawn(&env)?;

    // wait until `child` has been spawned
    thread::sleep(Duration::from_secs(1));

    let output = Command::new("visudo").output(&env)?;

    child.wait()?.assert_success()?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    assert_contains!(
        output.stderr(),
        "visudo: /etc/sudoers busy, try again later"
    );

    Ok(())
}

#[test]
fn passes_temporary_file_to_editor() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo "$@" > {LOGS_PATH}"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let args = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert_eq!("-- /etc/sudoers.tmp", args);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn temporary_file_owner_and_perms() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
ls -l /etc/sudoers.tmp > {LOGS_PATH}"#
            ))
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let ls_output = Command::new("cat").arg(LOGS_PATH).output(&env)?.stdout()?;

    assert!(ls_output.starts_with("-rwx------ 1 root root"));

    Ok(())
}

#[test]
fn saves_file_if_no_syntax_errors() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(format!(
                r#"#!/bin/sh
echo '{expected}' >> $2"#
            ))
            .chmod("100"),
        )
        .build()?;

    Command::new("rm")
        .args(["-f", ETC_SUDOERS])
        .output(&env)?
        .assert_success()?;

    Command::new("visudo").output(&env)?.assert_success()?;

    let sudoers = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, sudoers);

    Ok(())
}

#[test]
#[ignore = "gh657"]
fn stderr_message_when_file_is_not_modified() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(DEFAULT_EDITOR, TextFile(EDITOR_TRUE).chmod(CHMOD_EXEC))
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());
    assert_eq!(output.stderr(), "visudo: /etc/sudoers.tmp unchanged");

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn does_not_save_the_file_if_there_are_syntax_errors() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(expected)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh

echo 'this is fine' > $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());
    assert_contains!(output.stderr(), "syntax error");

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn editor_exits_with_a_nonzero_code() -> Result<()> {
    let expected = SUDOERS_ALL_ALL_NOPASSWD;
    let env = Env(SUDOERS_ALL_ALL_NOPASSWD)
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
exit 11",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(output.status().success());

    let actual = Command::new("cat")
        .arg(ETC_SUDOERS)
        .output(&env)?
        .stdout()?;

    assert_eq!(expected, actual);

    Ok(())
}

#[test]
fn temporary_file_is_deleted_during_edition() -> Result<()> {
    let env = Env("")
        .file(
            DEFAULT_EDITOR,
            TextFile(
                "#!/bin/sh
rm $2",
            )
            .chmod(CHMOD_EXEC),
        )
        .build()?;

    let output = Command::new("visudo").output(&env)?;

    assert!(!output.status().success());
    assert_eq!(Some(1), output.status().code());
    let stderr = output.stderr();
    assert_contains!(
        stderr,
        "visudo: unable to re-open temporary file (/etc/sudoers.tmp), /etc/sudoers unchanged"
    );

    Ok(())
}