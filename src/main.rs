use core::time::Duration;
use chrono::Local;
use std::thread::sleep;
use std::fs::File;
use std::io::{self, Write};
use serialport::{SerialPort, SerialPortType};
use crossterm::style::Print;
use crossterm::{cursor, queue};
use crossterm::terminal::{enable_raw_mode, Clear, ClearType};
use crossterm::event::{read, Event::Key, KeyCode, KeyEvent, poll};

fn main() {
    queue!(io::stdout(), Clear(ClearType::All)).unwrap();
    queue!(io::stdout(), cursor::Hide).unwrap();
    io::stdout().flush().unwrap();
    enable_raw_mode().unwrap();
    println!("\n\n\r");
    println!("******************** SENSA Device Controller ***************************\r");
    println!("\r");
    let mut command_buf = String::new();
    // For enabling and disabling data recording from the device
    let mut recording = false;
    let mut file: File = File::create("dummy_file").unwrap();
    let _ = std::fs::remove_file("dummy_file").unwrap();
    draw_commands_field(&mut command_buf);

    let port_name;
    let baud_rate = 115200;

    // Obtaining list of available ports and searching for the USB port with a CP2102 USB to UART Bridge Controller connected
    // This is the USB to UART Bridge Controller chip on the ESP32 in our device
    'outer: loop {
        let ports = serialport::available_ports().expect("Device not found!\r");
        for port in &ports {
            match &port.port_type {
                SerialPortType::UsbPort(device_info) =>  {
                    if device_info.product != Some(String::from("CP2102 USB to UART Bridge Controller")) {continue;}
                    port_name = port.port_name.clone();
                    println!("SENSA device found on port {}\r", port_name); 
                    break 'outer;
                },
                _ => (),
            }
        }
        println!("Device not found\r");
        // If device is not connected by the time the program is started then "Device not found"
        // will be output every 2 seconds until the device is connected
        // to do: change functionality here such that no device foun
        sleep(Duration::from_secs(2));
    }
    // Connecting to the previously found USB port
    let port = serialport::new(&port_name, baud_rate)
        .timeout(Duration::from_millis(10))
        .open();

    match port {
        // If port connects successfully
        Ok(mut port) => {
            println!("Connected to SENSA device\r");
            println!("Please enter command \"start\" to start recording the sensor data to a file\r");
            // Buffer for serial data after reading it from the port
            let mut serial_buf: Vec<u8> = vec![0; 1000];

            let mut received_data = Vec::new();
            println!("Receiving data on port {} at {} baud:\r", &port_name, &baud_rate);

            let mut first_read = true;
            loop {
                match port.read(serial_buf.as_mut_slice()) {
                    Ok(t) => {
                        io::stdout().write_all(&serial_buf[..t]).unwrap();
                        io::stdout().flush().unwrap();
                        draw_commands_field(&mut command_buf);

                        received_data.extend_from_slice(&serial_buf[..t]);
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
                    Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => {
                        eprintln!("Device disconnected"); 
                        std::process::exit(1);
                    },
                    Err(e) => {
                        eprintln!("{:?}", e); 
                        std::process::exit(1);
                    }
                }

                if received_data.contains(&b'-') {
                    let mut data_string: Vec<u8> = received_data.splice(..1+received_data.iter()
                                                                .position(|&x| x == b'-')
                                                                .unwrap(), [])
                                                                .collect();
                    if first_read{data_string.clear(); first_read = false; continue;}
                    let data_instance = format!("{},{}", Local::now().format("%H:%M:%S"), data_string.iter_mut()
                                                                                                          .map(|x| char::from(*x))
                                                                                                          .collect::<String>()
                                                                                                          .replace(">", "")
                                                                                                          .replace(" MQ3 : ", "")
                                                                                                          .replace(" MQ5 : ", "")
                                                                                                          .replace(" MQ131 : ", "")
                                                                                                          .replace(" MQ135 : ", "")
                                                                                                          .replace(" MP503 : ", "")
                                                                                                          .replace(" Temperature : ", "")
                                                                                                          .replace(" Humidity : ", "")
                                                                                                          .replace("-", ""));
                    data_string.clear();
                    if recording {
                        file.write_all(data_instance.as_bytes()).unwrap();
                        file.flush().unwrap();
                    }
                }
                
                process_commands(&mut command_buf, &mut recording, &mut file, port.as_mut());
                draw_commands_field(&mut command_buf);
            }
        }
        Err(e) => {
            eprintln!("Failed to open \"{}\". Error: {}", port_name, e);
            std::process::exit(1);
        }
    }
}


fn process_commands(command: &mut String, recording: &mut bool, file: &mut File, port: &mut dyn SerialPort) -> () {
    if poll(Duration::from_millis(1)).unwrap(){
        // It's guaranteed that `read` won't block, because `poll` returned
        // `Ok(true)`
        let event = read().unwrap();
        if let Key(KeyEvent {code,..}) = event {
            if let KeyCode::Char(x) = code {
                command.push(x);
                queue!(io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
                queue!(io::stdout(), cursor::MoveToColumn(0)).unwrap();
                io::stdout().flush().unwrap();             
            } else {
                if code == KeyCode::Enter {
                    if command.trim() == "start".to_string() {
                        if !*recording {
                            println!("Creating file\n\rStarting to record\n\rTo end the recording session, please use the \"stop\" command \n\r");
                            let file_name = format!("Session {} .csv", Local::now().format("%d-%m-%Y %H-%M-%S"));
                            *file = File::create(file_name).unwrap();
                            file.write_all(b"Time,MQ3,MQ5,MQ131,MQ135,MP503,Temperature,Humidity\n").unwrap();

                            *recording = true;
                        } else {
                            println!("A recording session is already in progress. Please enter the \"stop\" command to end it before attempting\r");
                            println!("to start another one\r");
                        }
                    }
                    else if command.trim() == "stop".to_string() {
                        if *recording {
                            println!("Ending the recording session\n\r");
                            *recording = false;
                        } else {
                            println!("There is no recording session in progress\r");
                        }
                    } else {
                        port.write(command.as_bytes()).expect("Write failed!");
                    }
                    command.clear();  
                }
                if code == KeyCode::Backspace {
                    command.pop();
                    queue!(io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
                    queue!(io::stdout(), cursor::MoveToColumn(0)).unwrap();
                    io::stdout().flush().unwrap();     
                }
            }
        }
    }
}


fn draw_commands_field(command: &mut String) {
    queue!(io::stdout(), cursor::SavePosition).unwrap();
    queue!(io::stdout(), cursor::MoveTo(0, 0)).unwrap();
    queue!(io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
    queue!(io::stdout(), Print("Command: ")).unwrap();
    queue!(io::stdout(), Print(command)).unwrap();
    queue!(io::stdout(), cursor::MoveTo(0, 1)).unwrap();
    queue!(io::stdout(), Clear(ClearType::CurrentLine)).unwrap();
    queue!(io::stdout(), Print("=========================================================================================================")).unwrap();
    queue!(io::stdout(), cursor::RestorePosition).unwrap();
    io::stdout().flush().unwrap();
}