use std::collections::HashMap;
use std::fs::File;
use std::hash::Hash;
fn save_events_to_file(events: &Vec<RecordedEvent>) -> std::io::Result<()> {
    let file = File::create("macro.json")?;
    serde_json::to_writer(file, events)?;
    Ok(())
}
use std::path::Path;
use std::time::Instant;

use evdev_rs::Device;
use evdev_rs::ReadFlag;
use evdev_rs::enums::EventCode;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json;
use std::time::Duration;
use uinput::event::Code;
use uinput::event::keyboard::Key;
use uinput::event::{Event, keyboard};
#[derive(Debug, Deserialize, Serialize)]
struct RecordedEvent {
    timestamp_ms: u128,
    key: EventCode,
    value: i32, // 0/1/2
}

const BREAK: EventCode = EventCode::EV_KEY(evdev_rs::enums::EV_KEY::KEY_KPMINUS);
const START: EventCode = EventCode::EV_KEY(evdev_rs::enums::EV_KEY::KEY_KPPLUS);
const REPLAY: EventCode = EventCode::EV_KEY(evdev_rs::enums::EV_KEY::KEY_STOPCD);
const LOOP_ITERATION: i32 = 10;
static KEY_MAP: Lazy<HashMap<u16, Key>> = Lazy::new(|| {
    let mut m = HashMap::new();

    for key in Key::iter_variants() {
        let code = key.code() as u16; // numeric key code
        m.insert(code, key);
    }

    m
});

//if i ever need to ill move everything away from main
//should proably have a more robust event file path querying system
fn main() {
    unsafe { std::env::set_var("RUST_BACKTRACE", "FULL") };
    let d = Device::new_from_path("/dev/input/event2").unwrap();
    //only start recoding when told to
    let file_paths = vec!["macro.json"];
    let mut events: Vec<RecordedEvent> = vec![];
    loop {
        //temporarily loop to listen to what keys to start pressing
        let ev = d.next_event(ReadFlag::NORMAL).map(|val| val.1);
        match ev {
            Ok(ev) => match ev.event_code {
                START => break,
                REPLAY => {
                    println!("Starting replay");
                    looped_replay(Some(LOOP_ITERATION), file_paths);
                    //this probably the dumbest way to get out of the loop
                    panic!("Finished playing the macro");
                }
                _ => continue,
            },
            Err(_e) => (),
        }
    }

    let start = Instant::now();
    loop {
        let ev = d.next_event(ReadFlag::NORMAL).map(|val| val.1);
        match ev {
            Ok(ev) => match ev.event_code {
                EventCode::EV_MSC(_s) => continue,
                EventCode::EV_SYN(_s) => continue,
                BREAK => {
                    break;
                }
                _ => {
                    let ms = start.elapsed().as_millis();
                    let val = ev.value;
                    if val == 2 {
                        continue;
                    }
                    let key_event = RecordedEvent {
                        timestamp_ms: ms,
                        key: ev.event_code,
                        value: val,
                    };
                    events.push(key_event);
                    println!("keycode:{} timestamp: {} val: {}", ev.event_code, ms, val);
                }
            },
            Err(_e) => (),
        }
    }
    match save_events_to_file(&events) {
        Ok(_) => {
            println!("Wrote macro to file");
        }
        Err(e) => {
            panic!("{}", e)
        }
    }
}
fn create_virtual_keyboard() -> uinput::Device {
    uinput::default()
        .expect("uinput not available")
        .name("rust-macro-kbd")
        .expect("failed to name")
        .event(Event::Keyboard(keyboard::Keyboard::All))
        .expect("failed to bind events")
        .create()
        .expect("failed to create virt keyboard")
}
struct ReplayableEvent {
    timestamp_ms: u128,
    key: uinput::event::keyboard::Key,
    value: i32,
}
//None = loop forever
fn looped_replay<P>(looped: Option<i32>, file_path: Vec<P>)
where
    P: AsRef<Path> + Eq + Hash + Clone,
{
    let mut preloaded: HashMap<P, Vec<ReplayableEvent>> = HashMap::new();
    for path in &file_path {
        let file = File::open(path).unwrap();
        let events: Vec<RecordedEvent> = serde_json::from_reader(file).unwrap();
        let converted: Vec<ReplayableEvent> = events
            .into_iter()
            .filter_map(|e| {
                evdev_to_uinput(&e.key).map(|key| ReplayableEvent {
                    timestamp_ms: e.timestamp_ms,
                    key,
                    value: e.value,
                })
            })
            .collect();
        preloaded.insert(path.clone(), converted);
    }

    let mut device = create_virtual_keyboard();
    match looped {
        Some(count) => {
            for _ in 0..count {
                replay_macro(&file_path, &mut device, &preloaded);
            }
        }
        None => loop {
            //offset to allow stopping of program
            std::thread::sleep(Duration::from_secs(1));
            replay_macro(&file_path, &mut device, &preloaded);
        },
    }
}
fn evdev_to_uinput(code: &EventCode) -> Option<Key> {
    if let EventCode::EV_KEY(k) = code {
        let raw: u16 = *k as u16;
        KEY_MAP.get(&raw).copied()
    } else {
        None
    }
}

fn replay_macro<P>(
    file_path: &Vec<P>,
    keyboard: &mut uinput::Device,
    preloaded: &HashMap<P, Vec<ReplayableEvent>>,
) where
    P: AsRef<Path> + Eq + Hash + Clone,
{
    for path in file_path {
        let events = preloaded.get(path).unwrap();
        replay_events(events, keyboard);
    }
}
fn replay_events(events: &[ReplayableEvent], keyboard: &mut uinput::Device) {
    if events.is_empty() {
        return;
    }
    let mut last_timestamp = events[0].timestamp_ms;
    for event in events {
        let delay_ms = event.timestamp_ms - last_timestamp;
        last_timestamp = event.timestamp_ms;
        //idk if this will ever happen but if delay_ms ever breaks the u64 cap
        //itll break the sleep line
        std::thread::sleep(Duration::from_millis(delay_ms.try_into().unwrap()));
        let key = event.key;
        match event.value {
            1 => keyboard.press(&key).unwrap(),
            0 => keyboard.release(&key).unwrap(),
            _ => (),
        }
        keyboard.synchronize().unwrap();
    }
}
