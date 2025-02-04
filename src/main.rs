use std::os::unix::net::UnixStream;
use std::io::{Read, Write};

use std::sync::LazyLock;

// Returns the directory where Hyprland sockets are
fn hyprland_runtime_dir() -> String {

    let instance = match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(inst) => inst,
        Err(std::env::VarError::NotPresent) => {
            panic!("Couldn't get socket path! (Is Hyprland running?)");
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            panic!("Corrupted Hyprland socket variable: Invalid unicode!");
        }
    };

    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => return dir + "/hypr/" + instance.as_str(),
        Err(std::env::VarError::NotPresent) => {
            println!("Couldn't get XDG_RUNTIME_DIR");
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            println!("Corrupted XDG_RUNTIME_DIR");
        }
    };

    if let Ok(uid) = std::env::var("UID") {
        return "/run/user/".to_owned() + uid.as_str() + "/hypr/" + instance.as_str();
    } else {
        println!("Couldn't get UID");
        return "/run/user/100/hypr/".to_owned() + instance.as_str();
    }
}

// LazyLock sets the value the first time it is used, and then can't be set again
static RUN_DIR: LazyLock<String> = LazyLock::new(|| {hyprland_runtime_dir()});

fn run_hypr_command(command: &str) -> serde_json::Value {
    let mut command_stream = UnixStream::connect(RUN_DIR.to_string() + "/.socket.sock").expect("Couldn't open socket.sock");
    command_stream.write_all(command.as_bytes()).expect("Couldn't write to stream");
    let mut response_buf = [0; 4096];
    let num_read = command_stream.read(&mut response_buf).expect("Couldn't read from command stream");
    // let response = String::from_utf8(response_buf[..num_read].to_vec()).expect("Couldn't parse command stream as string");
    return serde_json::from_slice(&response_buf[..num_read]).unwrap_or_default();
}

fn zip_workspaces(ws_num: u64, except_window: Option<&str>) {
    let serde_json::Value::Array(clients) = run_hypr_command("j/clients") else { panic!("Couldn't read clients from hyprctl")};
    println!("Running a zip_workspaces check for {}", ws_num);
    for client in clients {
        let client_ws = client["workspace"]["id"].as_u64().expect("Couldn't read workspace id");
        let client_addr = client["address"].as_str().expect("Couldn't read client address");
        let except_window = except_window.unwrap_or_default();
        if client_ws >= ws_num && client_addr != except_window {
            println!("Client is on later workspace! Moving {} from workspace {} to {}",
                client["title"].as_str().unwrap_or("Unknown window"),
                client_ws,
                client_ws - 1
            );
            run_hypr_command(&format!("dispatch movetoworkspacesilent {},address:{}", client_ws - 1, client_addr));
        }
    }
}

fn unzip_workspaces(ws_num: u64, except_window: Option<&str>) {
    let serde_json::Value::Array(clients) = run_hypr_command("j/clients") else { panic!("Couldn't read clients from hyprctl")};
    for client in clients {
        let client_ws = client["workspace"]["id"].as_u64().expect("Couldn't read workspace id");
        let client_addr = client["address"].as_str().expect("Couldn't read client address");
        let except_window = except_window.unwrap_or_default();
        if client_ws >= ws_num && client_addr != except_window {
            println!("Client is on later workspace! Moving {} from workspace {} to {}",
                client["title"].as_str().unwrap_or("Unknown window"),
                client_ws,
                client_ws + 1
            );
            run_hypr_command(&format!("dispatch movetoworkspacesilent {},address:{}", client_ws + 1, client_addr));
        }
    }
}

fn process_event(event: &str) {
    let mut event = event.split(">>");
    let eventtype = event.next().unwrap_or_default();
    if eventtype == "destroyworkspacev2" {
        let mut event_params = event.next().unwrap_or_default().split(',');
        let ws_id = event_params.next().unwrap_or_default().parse().expect("Couldn't get workspace id from destroyworkspacev2 event");
        zip_workspaces(ws_id, None);
        let active_ws_id = run_hypr_command("j/activeworkspace")["id"].as_u64().expect("Couldn't get active workspace id");
        if active_ws_id > ws_id {
            run_hypr_command("dispatch workspace -1");
        }
    } else if eventtype == "closewindow" {
        let active_ws = run_hypr_command("j/activeworkspace");
        let ws_id = active_ws["id"].as_u64().expect("Couldn't get active workspace id");
        let windows = active_ws["windows"].as_u64().expect("Couldn't get number of windows");
        if windows == 0 {
            if ws_id == get_max_ws_num() {
                run_hypr_command("dispatch workspace -1");
            } else {
                zip_workspaces(ws_id, None);
            }
        }
    }
}

fn get_max_ws_num() -> u64{
    let serde_json::Value::Array(workspaces) = run_hypr_command("j/workspaces") else { panic!("Couldn't read workspaces from hyprctl")};
    let mut max = 0;
    for ws in workspaces {
        let id = ws["id"].as_u64().expect("Couldn't read workspace id");
        if id > max {
            max = id;
        }
    }
    return max;

}

fn daemon() {
    println!("{}", RUN_DIR.to_string());
    let mut event_stream = UnixStream::connect(RUN_DIR.to_string() + "/.socket2.sock").expect("Couldn't open socket2.sock");
    println!("Connected to event_stream");
    loop {
        let mut response_buf = [0; 4096];
        let num_read = match event_stream.read(&mut response_buf) {
            Ok(data) => data,
            Err(err)  => {
                println!("Error reading event stream: {}", err);
                break;
            }
        };

        if num_read == 0 {
            println!("num_read was 0");
            break;
        }

        let string = String::from_utf8_lossy(&response_buf[..num_read]);

        let responses = string.split('\n');
        for line in responses {
            if line.is_empty() {
                continue;
            }
            println!("New event: {}", line);
            process_event(line);
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(50));
    let mut response_buf = [0; 2048];
    let _ = event_stream.read(&mut response_buf);
    println!("Finished reading");
}

enum Direction {
    Left,
    Right
}

fn move_window(dir: Direction, create: bool) {
    let activeworkspace = run_hypr_command("j/activeworkspace");
    let ws_num = activeworkspace["id"].as_u64().expect("Couldn't get activeworkspace id");

    let activewindow = run_hypr_command("j/activewindow");
    let client_addr = activewindow["address"].as_str().expect("Couldn't get active window address");
    match dir {
        Direction::Left => {
            if create {
                println!("Moving left with create");
                unzip_workspaces(ws_num, Some(client_addr));
            } else {
                println!("Moving left");
                run_hypr_command("dispatch movetoworkspace -1");
                //run_hypr_command("dispatch setfullscreenstate 0");
            }
        }
        Direction::Right => {
            if !(!create && ws_num == get_max_ws_num()) {
                println!("Moving right");
                run_hypr_command("dispatch movetoworkspace +1");
                //run_hypr_command("dispatch setfullscreenstate 0");
            }
            if create {
                println!("Moving right with create");
                unzip_workspaces(ws_num + 1, Some(client_addr));
            }
        }
    }
}

fn move_to_workspace_cmd(mut args_iter: std::env::Args) {
    let Some(dir) = args_iter.next() else {
        println!("Please provide a direction");
        return;
    };
    let dir = match dir.as_str() {
        "left" => Direction::Left,
        "right" => Direction::Right,
        _ => {
            println!("Invalid direction provided");
            return;
        }
    };

    let create = match args_iter.next() {
        Some(s) => s == "create",
        None => false
    };

    move_window(dir, create);
}

fn workspace_cmd(mut args_iter: std::env::Args) {
    let Some(dir) = args_iter.next() else {
        println!("Please provide a direction");
        return;
    };
    let dir = match dir.as_str() {
        "left" => "-1",
        "right" => "+1",
        _ => {
            println!("Invalid direction provided");
            return;
        }
    };

    let ws_id = run_hypr_command("j/activeworkspace")["id"].as_u64().expect("Couldn't get workspace id");

    if !(ws_id >= get_max_ws_num() && dir == "+1") {
        let movecmd = format!("dispatch workspace {}", dir);
        println!("{}", &movecmd);
        run_hypr_command(&movecmd);
    }
}

fn main() {
    let mut args_iter = std::env::args();
    args_iter.next();
    let Some(command) = args_iter.next() else {
        println!("Please provide a command");
        // TODO: help menu
        return;
    };

    match command.as_str() {
        "daemon" => {daemon();}
        "movetoworkspace" => {move_to_workspace_cmd(args_iter);}
        "workspace" => {workspace_cmd(args_iter);}
        _ => { println!("Invalid command provided"); }
    };

}
