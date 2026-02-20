use bitvec::{order::Msb0, view::BitView};
use raw_tty::IntoRawMode;
use renderer::{
    hr_bw_display::{HighResBWScreen, Res},
    term_display::TermStatusLine,
    traits::RenderTarget,
};
use std::{
    env,
    io::{self, Read},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use crate::frame::{Frame, ToFrames};

mod frame;

const WIDTH: usize = 480;
const HEIGHT: usize = 360;
const APPLE: &[u8] = include_bytes!("../assets/apple480.raw");
// const APPLE: &[u8] = include_bytes!("../assets/apple_short.raw");
const FPS: u64 = 30;

fn play_apple(scale: usize, fps: u64, res: Res) -> io::Result<()> {
    let w = WIDTH.div_ceil(scale);

    let mut screen = HighResBWScreen::new(w, res);
    let mut statusline = TermStatusLine::new(w);

    screen.init()?;
    statusline.init()?;

    let (frames_tx, frames_rx) = mpsc::channel::<Frame>();
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
        let _ = APPLE
            .view_bits::<Msb0>()
            .iter()
            .map(|b| *b)
            .to_frames(WIDTH, HEIGHT, scale)
            .try_for_each(|frame| frames_tx.send(frame));
    });

    let mut frame_st = Instant::now();
    let start = Instant::now();

    let sleep = Duration::from_micros(1_000_000 / fps);

    while let Ok(frame) = frames_rx.recv() {
        frame.draw_frame_to(&mut screen)?;
        // statusline.draw("Press q to exit!".chars())?;

        if stop_rx.try_recv().is_ok() {
            break;
        }

        let delta = frame_st.elapsed();
        statusline.draw(format!("dt: {}us     ", delta.as_micros()).chars())?;

        thread::sleep(sleep.saturating_sub(delta));
        frame_st = Instant::now();
    }

    let total = start.elapsed();

    input.join().unwrap();

    screen.exit()?;
    statusline.exit()?;

    println!("Total: {:.2}", total.as_secs_f64());

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut scale: usize = 1;
    let mut fps: u64 = FPS;
    let mut res: Res = Res::High;

    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        let Some(val) = iter.next() else {
            continue;
        };

        match arg.as_str() {
            "-s" | "--scale" => {
                scale = val.parse().unwrap();
                if !(0..=100).contains(&scale) {
                    eprintln!("Scale is not in range [0, 100]!");
                    return;
                }
            }
            "-r" | "--rate" => {
                fps = val.parse().unwrap();
                if !(10..=120).contains(&fps) {
                    eprintln!("Framerate is not in range [10, 120]!");
                    return;
                }
            }
            "-q" | "--quality" => match val.as_str() {
                "extra" | "e" => res = Res::Extra,
                "high" | "h" => res = Res::High,
                "low" | "l" => res = Res::Low,
                _ => {
                    eprintln!("Invalid quality, not in {{e, h, l, extra, high, low}}");
                    return;
                }
            },
            _ => {}
        }
    }

    play_apple(scale, fps, res).unwrap();
}
