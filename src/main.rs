use std::{
    collections::HashMap, env, io::{self, Read, Write}, process::{Command, Stdio, exit}, thread, time
};

use image::{
    imageops::{resize, FilterType},
    RgbImage
};

use rodio::{Decoder, Player};
use std::fs::File;

use crossterm::terminal;

const CHARS: [(char, f64); 32] = [('˙', 0.014086057004511938), ('`', 0.024100363156157142), ('-', 0.0357653791130186), (',', 0.045339495983272804), (';', 0.06272697259821723), ('"', 0.0719709475074282), ('◌', 0.08275558490150764), ('∙', 0.09381534059645646), ('÷', 0.10630571145592606), ('<', 0.11846594035435237), ('◇', 0.12941564872895345), ('l', 0.1416859249477275), ('s', 0.15203037306041595), ('T', 0.1626499394739738), ('V', 0.17888191922526683), ('h', 0.18796082315395618), ('e', 0.20072631231429514), ('▪', 0.2118961153295917), ('6', 0.22400132056784414), ('E', 0.236381644106966), ('Φ', 0.24595576097722022), ('Ψ', 0.2585561791570375), ('◍', 0.26653460988224936), ('B', 0.27941014636293604), ('Ж', 0.2936062506877958), ('Ю', 0.3040607461208319), ('@', 0.3333883569935072), ('●', 0.39072301089468464), ('▒', 0.49257180587652694), ('■', 0.4980191482337405), ('▓', 0.8037856278199625), ('█', 1.0)];

const FPS: usize = 30;

fn load_video(path: &str, width: u16, height: u16) -> io::Result<Vec<RgbImage>> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-i", path,
            "-vf", &format!("fps={FPS},scale={width}:{height}"),
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .spawn()?;

    let mut stdout = child.stdout.take().unwrap();
    let frame_size = width as usize * height as usize * 3;
    let mut frames = Vec::new();

    loop {
        let mut buffer = vec![0u8; frame_size];

        match stdout.read_exact(&mut buffer) {
            Ok(()) => {
                frames.push(
                    RgbImage::from_raw(width as u32, height as u32, buffer).unwrap()
                );
            }
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e),
        }
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(io::Error::other("FFmpeg failed"));
    }

    Ok(frames)
}

fn get_closest_char(target: f64, fullwidth_only: bool) -> char {
    if fullwidth_only {
        return '█';
    }
    let mut best = 256.0;
    let mut best_ch = '\0';

    for (ch, density) in CHARS {
        let diff = (density - target).abs();
        if diff < best {
            best = diff;
            best_ch = ch;
        }
    }

    assert_ne!(best_ch, '\0');
    return best_ch;
}

fn color(mut r: u8, mut g: u8, mut b: u8, fullwidth_only: bool) -> String {
    let sum_colors = r.saturating_add(g).saturating_add(b);
    if sum_colors > 0 && !fullwidth_only {
        // Not much goes smaller than a period character (roughly 5% ink) so really dark colors should be boosted less
        // Basically anything works as long as 0 -> 0 and (255*3) -> (255*3)
        let m = 255.0 / r.max(g).max(b) as f64 * ((sum_colors as f64) * (765.0 as f64).powf(3.0)).powf(0.25) / (255.0 * 3.0);
        r = (r as f64 * m) as u8;
        g = (g as f64 * m) as u8;
        b = (b as f64 * m) as u8;
    }
    return format!("\x1b[38;2;{};{};{}m", r, g, b);
}

fn frame_to_str(raw_img: &RgbImage, width: u32, height: u32, map: &HashMap<i32, char>, fullwidth_only: bool) -> String {
    let img = resize(raw_img, width, height, FilterType::Triangle);

    let mut res = String::new();

    for row in 0..height {
        for col in 0..width {
            let [r, g, b] = img[(col, row)].0;
            res.push_str(&color(r, g, b, fullwidth_only));
            res.push(map[&(r as i32 + g as i32 + b as i32)]);
        }
        if row != height - 1 {
            res.push('\n');
        }
    }

    return res;
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 3 {
        eprintln!("Usage ./path/to/exe <image/video> <path>");
        panic!();
    }
    let mode = args.get(1).unwrap().to_lowercase();
    let file = args.get(2).unwrap();

    assert!(mode == "image" || mode == "video");

    let mut buf = String::new();
    print!("Fullwidth only (1) or ASCII (2) > ");
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut buf).unwrap();
    let choice = buf.chars().nth(0).unwrap();
    assert!(choice == '1' || choice == '2');

    let fullwidth_only = choice == '1';

    let (cols, lines) = terminal::size().unwrap();
    let map: HashMap<i32, char> = (0..=(255*3)).map(|v| (v, get_closest_char(v as f64 / 3.0 / 255.0, fullwidth_only))).collect();

    if mode == "image" {
        let frame = &image::open(file).unwrap().to_rgb8();
        println!("{}", frame_to_str(&frame, cols as u32, lines as u32, &map, fullwidth_only));

        exit(0);
    }

    let frames = load_video(file, cols, lines).unwrap();

    let mut stdout = io::stdout().lock();

    // Sound
    let mut handle = rodio::DeviceSinkBuilder::open_default_sink().unwrap();
    handle.log_on_drop(false);
    let file = File::open(file).unwrap();
    let source = Decoder::try_from(file).unwrap();
    let player = Player::connect_new(handle.mixer());
    player.append(source);

    let start = time::Instant::now();
    let mut next_frame = time::Instant::now();
    let frame_time = time::Duration::from_millis(1000 / FPS as u64);

    let mut displayed_frames = 0 as u64;

    print!("\x1b[2J");
    while !player.empty() {
        let time = player.get_pos().as_secs_f64();
        let frame = (time * FPS as f64).round() as usize;
        if frame >= frames.len() {
            break;
        }
        write!(stdout, "\x1b[?2026h\x1b[H").unwrap(); // Freeze rendering, home
        write!(stdout, "{}", frame_to_str(&frames[frame], cols as u32, lines as u32, &map, fullwidth_only)).unwrap();
        write!(stdout, "\x1b[?2026l").unwrap(); // Unfreeze rendering
        stdout.flush().unwrap();
        displayed_frames += 1;
        next_frame += frame_time;
        thread::sleep(next_frame.saturating_duration_since(time::Instant::now()));
    }

    let end = time::Instant::now();
    let elapsed = end.duration_since(start).as_secs();

    print!("\x1b[0m");
    println!("{} s elapsed", elapsed);
    println!("{:.2} FPS", displayed_frames as f64 / elapsed as f64);
}
