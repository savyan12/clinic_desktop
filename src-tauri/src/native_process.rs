use std::io::Read;
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

fn capture(pipe: Option<impl Read + Send + 'static>) -> Receiver<std::io::Result<Vec<u8>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = pipe.map_or(Ok(()), |mut pipe| pipe.read_to_end(&mut bytes).map(|_| ()));
        let _ = sender.send(result.map(|_| bytes));
    });
    receiver
}

pub fn output(command: &mut Command, timeout: Duration, operation: &str) -> Result<Output, String> {
    let mut child = command
        .spawn()
        .map_err(|error| format!("{operation}: {error}"))?;
    let stdout = capture(child.stdout.take());
    let stderr = capture(child.stderr.take());
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => {
                terminate_owned_child(&mut child);
                return Err(format!("{operation}: انتهت المهلة المسموح بها وأُوقفت العملية التي بدأها النظام. أعد المحاولة بعد فحص الخدمات أو وحدة التخزين."));
            }
            Err(error) => {
                terminate_owned_child(&mut child);
                return Err(format!("{operation}: تعذر متابعة العملية: {error}"));
            }
        }
    };
    let read = |receiver: Receiver<std::io::Result<Vec<u8>>>| {
        receiver
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| format!("{operation}: لم تُغلق قناة نتيجة العملية في الوقت المحدد."))?
            .map_err(|error| format!("{operation}: تعذر قراءة نتيجة العملية: {error}"))
    };
    Ok(Output {
        status,
        stdout: read(stdout)?,
        stderr: read(stderr)?,
    })
}

pub fn terminate_owned_child(child: &mut Child) {
    #[cfg(windows)]
    {
        // Kill only the process tree spawned by this operation, never by name.
        let mut command = Command::new("taskkill.exe");
        crate::runtime::hide_console_window(&mut command);
        if let Ok(mut killer) = command
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            let deadline = Instant::now() + Duration::from_secs(5);
            while killer.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(20));
            }
            let _ = killer.kill();
            let _ = killer.wait();
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;
    fn helper(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        crate::runtime::hide_console_window(&mut command);
        command
            .args([
                "--ignored",
                "--exact",
                "native_process::tests::child_process_helper",
                "--nocapture",
            ])
            .env("CLINIC_NATIVE_PROCESS_TEST", mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    #[test]
    fn command_deadline_terminates_a_stuck_owned_process() {
        let start = Instant::now();
        assert!(output(
            &mut helper("sleep"),
            Duration::from_millis(300),
            "test timeout"
        )
        .is_err());
        assert!(start.elapsed() < Duration::from_secs(8));
    }
    #[test]
    fn large_output_is_drained_without_a_pipe_deadlock() {
        let result = output(&mut helper("output"), Duration::from_secs(8), "test output").unwrap();
        assert!(result.status.success());
        assert!(result.stdout.len() >= 200_000);
    }
    #[test]
    #[ignore = "subprocess fixture; invoked only by native process tests"]
    fn child_process_helper() {
        match std::env::var("CLINIC_NATIVE_PROCESS_TEST").as_deref() {
            Ok("sleep") => thread::sleep(Duration::from_secs(30)),
            Ok("output") => println!("{}", "x".repeat(200_000)),
            _ => {}
        }
    }
}
