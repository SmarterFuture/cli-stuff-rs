use bitvec::{order::Msb0, view::BitView};
use raw_tty::IntoRawMode;
use std::{
    env,
    io::{self, Read, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::frame::ToFrames;

mod chunk_iter;
mod frame;

const WIDTH: usize = 480;
const HEIGHT: usize = 360;
const APPLE: &[u8] = include_bytes!("../assets/apple480.raw");
// const APPLE: &[u8] = include_bytes!("../assets/apple_short.raw");
const FPS: u64 = 30;

fn play_apple(scale: usize, fps: u64) -> io::Result<()> {
    print!("\x1B[?1049h");
    print!("\x1B[?25l");
    print!("\x1B[2J\x1B[H");
    io::stdout().flush()?;

    let (frames_tx, frames_rx) = mpsc::channel::<String>();
    let (stop_tx, stop_rx) = mpsc::channel::<bool>();

    let input = thread::spawn(move || {
        let mut raw_stdin = io::stdin().into_raw_mode().unwrap();
        let mut buf = [0u8; 1];

        loop {
            raw_stdin.read_exact(&mut buf).unwrap();

            if buf[0] == b'q' || buf[0] == 3 {
                stop_tx.send(true).unwrap();
                break;
            }
        }
    });

    thread::spawn(move || {
        APPLE
            .view_bits::<Msb0>()
            .iter()
            .map(|b| *b)
            .to_frames(WIDTH, HEIGHT, scale)
            .for_each(|frame| frames_tx.send(frame).unwrap());
    });

    let mut frame_st = Instant::now();
    let start = Instant::now();

    let sleep = Duration::from_micros(1_000_000 / fps);

    while let Ok(frame) = frames_rx.recv() {
        print!("\x1B[H");
        print!("{}", frame);
        print!("Press q to exit!");
        io::stdout().flush()?;

        if stop_rx.try_recv().is_ok() {
            break;
        }

        let delta = frame_st.elapsed();
        print!(" dt: {}us        ", delta.as_micros());
        io::stdout().flush()?;

        thread::sleep(sleep.saturating_sub(delta));
        frame_st = Instant::now();
    }

    let total = start.elapsed();

    input.join().unwrap();

    print!("\x1B[?1049l");
    print!("\x1b[?25h");
    io::stdout().flush()?;

    println!("Total: {:.2}", total.as_secs_f64());

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut scale: usize = 1;
    let mut fps: u64 = FPS;

    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-s" | "--scale" => {
                if let Some(val) = iter.next() {
                    scale = val.parse().unwrap();
                    if !(0..=100).contains(&scale) {
                        eprintln!("Scale is not in range [0, 100]!");
                        return;
                    }
                }
            }
            "-r" | "--rate" => {
                if let Some(val) = iter.next() {
                    fps = val.parse().unwrap();
                    if !(10..=120).contains(&fps) {
                        eprintln!("Framerate is not in range [10, 120]!");
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    play_apple(scale, fps).unwrap();
}
