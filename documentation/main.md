1. *use clap::{Parser, Subcommand};* - This turns the rust code into fully functional terminal command automatically generating the command-line interface, argument parser, and user help menus.This is like generating our own git or cargo

2. *use hollow_signal::{
  client::{self, Connection}, 
  protocol::{COLOR_NAMES, PORT},
  server::Server,
};* - hollow_signal is likely a Rust library or a module in your project that contains networking and game-related code. 2nd line represents two things self is the client module itself and the Connection means the connection inside the module typically represents a client connection. PORT could specify the network port used by the game. COLOR_NAMES could store names for colors used in the game or its network protocol. 4th line  imports Server from the server module. It might be a struct responsible for accepting connections, managing connected players, or coordinating multiplayer game state.

*hollow_signal*

**client** - Client-side functionality and Connection

**protocol** - Shared networking definitions, possibly PORT and COLOR_NAMES

**server** - Server-side functionality and Server

3. *use std::{
  io::{self, Write},
  net::{SocketAddr, ToSocketAddrs},
  sync::atomic::AtomicBool,
  };* ---> **io** — Handles input/output and I/O errors.
  
  **Write** — Writes data to an output stream.
  
  **SocketAddr** — Represents an IP address and port, e.g. 127.0.0.1:8080.
  
  **ToSocketAddrs** — Converts hostnames like localhost:8080 into socket addresses.
  
  **AtomicBool** — A thread-safe boolean (true/false), useful for signaling a server to stop.
