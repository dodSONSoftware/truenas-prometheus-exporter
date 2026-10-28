//! Test-only executable, bind-mounted into validation containers; never shipped.
use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::TcpStream,
    path::Path,
    time::Duration,
};

fn require(condition: bool, message: &str) -> Result<(), Box<dyn Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let status = fs::read_to_string("/proc/self/status")?;
    let field = |name: &str| -> Result<&str, Box<dyn Error>> {
        status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .ok_or_else(|| "missing process field".into())
    };
    for key in ["Uid:", "Gid:"] {
        let ids: Vec<_> = field(key)?.split_whitespace().collect();
        require(ids == ["10001"; 4], "unexpected user/group identity")?;
    }
    for key in ["CapInh:", "CapPrm:", "CapEff:", "CapBnd:", "CapAmb:"] {
        require(
            u64::from_str_radix(field(key)?.trim(), 16)? == 0,
            "capabilities remain enabled",
        )?;
    }
    require(
        field("NoNewPrivs:")?.trim() == "1",
        "no-new-privileges is disabled",
    )?;
    let path = format!("/tmp/truenas-runtime-write-check-{}", std::process::id());
    match OpenOptions::new().create_new(true).write(true).open(&path) {
        Err(error) if error.raw_os_error() == Some(30) => {}
        Ok(_) => {
            fs::remove_file(path)?;
            return Err("root filesystem is writable".into());
        }
        Err(_) => return Err("could not establish a read-only filesystem".into()),
    }
    for path in [
        "/bin/sh",
        "/bin/bash",
        "/usr/bin/apt",
        "/usr/bin/apt-get",
        "/usr/bin/dpkg",
        "/usr/bin/perl",
        "/usr/bin/mount",
    ] {
        require(
            !Path::new(path).try_exists()?,
            "unnecessary runtime tool is present",
        )?;
    }
    let license = fs::read_to_string("/usr/share/doc/truenas-exporter/LICENSE")?;
    require(
        license.contains("Copyright (c) 2026 dodson Software ( dodson labs )"),
        "missing license",
    )?;
    require(
        fs::metadata("/etc/ssl/certs/ca-certificates.crt")?.len() > 0,
        "missing CA bundle",
    )?;
    for argument in std::env::args().skip(1) {
        if argument == "--unready" || argument == "--targets" {
            let mut stream = TcpStream::connect("127.0.0.1:9110")?;
            stream.set_read_timeout(Some(Duration::from_secs(3)))?;
            stream.set_write_timeout(Some(Duration::from_secs(3)))?;
            let path = if argument == "--unready" {
                "/readyz"
            } else {
                "/metrics"
            };
            stream.write_all(
                format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )?;
            let mut response = Vec::new();
            stream.read_to_end(&mut response)?;
            if argument == "--unready" {
                require(
                    response.starts_with(b"HTTP/1.1 503 "),
                    "outage readiness must fail",
                )?;
            } else {
                require(
                    response.starts_with(b"HTTP/1.1 200 "),
                    "metrics unavailable",
                )?;
                let text = String::from_utf8(response)?;
                for target in ["alpha", "beta"] {
                    require(
                        text.contains(&format!("truenas_api_connected{{target=\"{target}\"}} 0")),
                        "target operational metrics missing",
                    )?;
                }
            }
        } else {
            require(
                !fs::read(argument)?.is_empty(),
                "test credential/CA mount is unreadable or empty",
            )?;
        }
    }
    println!("runtime security probe passed");
    Ok(())
}
