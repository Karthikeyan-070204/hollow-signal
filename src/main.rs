use clap::{Parser, Subcommand};
use hollow_signal::{
    client::{self, Connection},
    protocol::{COLOR_NAMES, PORT},
    server::Server,
};
use std::{
    io::{self, Write},
    net::{SocketAddr, ToSocketAddrs},
    sync::atomic::AtomicBool,
};

#[derive(Parser)] //gives your struct the ability to parse terminal arguments
#[command( //tells it what the application is called, its version, and its description.
    name = "hollow-signal",
    version,
    about = "A fog-soaked terminal LAN deathmatch. No command opens the setup menu."
)]
// Struct is used to group data, Args stores all the command-line arguments supplied by the user.
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
    /// Reduce atmospheric text and effects; gameplay fog stays unchanged.
    #[arg(long, global = true)]
    calm: bool,
    /// Terminal draw rate. Server simulation always runs at 30 Hz.
    #[arg(long,global=true,default_value_t=30,value_parser=clap::value_parser!(u16).range(15..=60))]
    fps: u16,
}
#[derive(Subcommand)]
enum Command {
    /// Host an arena AND play on this computer.
    Host {
        #[arg(long, default_value = "0.0.0.0:35353")]
        bind: String,
        #[arg(long, default_value = "Wanderer")]
        name: String,
        #[arg(long,default_value="cyan",value_parser=COLOR_NAMES)]
        color: String,
    },
    /// Join a host by LAN IP (port defaults to 35353).
    Join {
        address: String,
        #[arg(long, default_value = "Wanderer")]
        name: String,
        #[arg(long,default_value="amber",value_parser=COLOR_NAMES)]
        color: String,
    },
    /// Run a headless dedicated server. Ctrl+C stops it.
    Server {
        #[arg(long, default_value = "0.0.0.0:35353")]
        bind: String,
    },
}
fn ask(prompt: &str, default: &str) -> io::Result<String> {
    print!("{prompt} [{default}]: ");
    io::stdout().flush()?;
    let mut s = String::new();
    if io::stdin().read_line(&mut s)? == 0 {
        return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Input closed"));
    }
    let s = s.trim();
    Ok(if s.is_empty() {
        default.to_owned()
    } else {
        s.to_owned()
    })
}
fn menu() -> io::Result<Command> {
    println!("\nH O L L O W  S I G N A L\nWard 09 / LAN deathmatch / 5-minute rounds\n");
    let mode = loop {
        let s = ask("[H]ost & play or [J]oin", "H")?;
        if s.eq_ignore_ascii_case("h") || s.eq_ignore_ascii_case("j") {
            break s;
        }
        println!("Choose H or J.");
    };
    let name = ask("Your name (up to 16 ASCII letters/numbers)", "Wanderer")?;
    println!("Colours: {}", COLOR_NAMES.join(", "));
    let color = loop {
        let c = ask("Character colour", "cyan")?.to_ascii_lowercase();
        if COLOR_NAMES.contains(&c.as_str()) {
            break c;
        }
        println!("Choose one of the listed colours.");
    };
    if mode.eq_ignore_ascii_case("h") {
        Ok(Command::Host {
            bind: format!("0.0.0.0:{PORT}"),
            name,
            color,
        })
    } else {
        Ok(Command::Join {
            address: ask("Host LAN IP (for example 192.168.1.10)", "127.0.0.1")?,
            name,
            color,
        })
    }
}
fn resolve(address: &str) -> io::Result<SocketAddr> {
    let addr = if let Ok(ip) = address.parse::<std::net::IpAddr>() {
        SocketAddr::new(ip, PORT).to_string()
    } else if address.contains(':') {
        address.to_owned()
    } else {
        format!("{address}:{PORT}")
    };
    addr.to_socket_addrs()?
        .next()
        .ok_or_else(|| io::Error::other("Address did not resolve"))
}
fn run() -> io::Result<()> {
    let args = Args::parse();
    let command = match args.command {
        Some(c) => c,
        None => menu()?,
    };
    match command {
        Command::Server { bind } => {
            let server = Server::bind(&bind)?;
            println!(
                "HOLLOW SIGNAL server listening on {} / UDP\n8 players / 5-minute rounds. Ctrl+C stops the server.",
                server.local_addr()?
            );
            server.run(&AtomicBool::new(false))
        }
        Command::Host { bind, name, color } => {
            let server = Server::bind(&bind)?;
            let mut address = server.local_addr()?;
            if address.ip().is_unspecified() {
                address.set_ip(if address.is_ipv4() {
                    "127.0.0.1".parse().unwrap()
                } else {
                    "::1".parse().unwrap()
                });
            }
            let _server = server.background();
            let conn = Connection::connect(
                address,
                &name,
                COLOR_NAMES.iter().position(|c| *c == color).unwrap_or(0) as u8,
            )?;
            client::play(
                conn,
                "HOST / share your LAN IPv4 address",
                args.calm,
                args.fps,
            )
        }
        Command::Join {
            address,
            name,
            color,
        } => {
            let address = resolve(&address)?;
            println!("Connecting to {address} ...");
            let conn = Connection::connect(
                address,
                &name,
                COLOR_NAMES.iter().position(|c| *c == color).unwrap_or(0) as u8,
            )?;
            client::play(conn, &address.to_string(), args.calm, args.fps)
        }
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("\nHOLLOW SIGNAL: {e}");
        std::process::exit(1);
    }
}
