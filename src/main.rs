use std::{
    collections::HashMap, io::{self, Read, Write}, process::{Command, Stdio, exit}, thread, time
};

use image::{
    imageops::{resize, FilterType},
    RgbImage
};

use rodio::{Decoder, Player};
use std::fs::File;

use clap::{Parser, ValueEnum};

use crossterm::terminal;

const ASCII_CHARS: [(char, f64); 52] = [('.', 0.05215382076250206), ('`', 0.07228915662650601), (':', 0.10430764152500412), ('-', 0.10727842878362767), (',', 0.1359960389503218), (';', 0.18814985971282389), ('"', 0.2158772074599769), ('~', 0.22313913186994552), ('^', 0.23106123122627495), ('!', 0.24508994883644167), ('_', 0.2525169169830005), ('/', 0.31292292457501236), ('r', 0.31473840567750455), ('(', 0.335368872751279), (')', 0.33784452880013205), ('<', 0.3553391648786928), ('?', 0.3606205644495792), ('v', 0.37266875722066345), ('x', 0.3952797491335204), ('c', 0.4048522858557518), ('l', 0.4249876217197557), ('i', 0.4348902459151675), ('L', 0.4438026076910381), ('s', 0.4560158441987126), ('y', 0.47186004291137146), ('t', 0.48506354183858724), ('T', 0.4878692853606206), ('o', 0.5098201023271166), ('V', 0.5365571876547285), ('2', 0.5520712988942069), ('h', 0.5637894041921109), ('a', 0.5839247400561148), ('3', 0.5888760521538208), ('e', 0.6020795510810364), ('A', 0.6095065192275954), ('q', 0.6220498432084502), ('$', 0.6364086482917973), ('K', 0.6540683281069483), ('b', 0.6586895527314738), ('6', 0.6718930516586895), ('O', 0.6803102822247895), ('0', 0.6938438686251857), ('E', 0.7090278923914838), ('g', 0.7166199042746327), ('D', 0.7270176596798151), ('R', 0.7466578643340485), ('M', 0.7560653573196897), ('W', 0.7575507509490015), ('N', 0.7796666116520876), ('#', 0.7851130549595643), ('B', 0.8380920944050172), ('@', 1.0)];
const MIXED_CHARS: [(char, f64); 4] = [('░', 0.182348409816221), ('▒', 0.49257180587652694), ('▓', 0.8037856278199625), ('█', 1.0)];

#[derive(Clone, ValueEnum, PartialEq)]
enum DisplayType {
    Image,
    Animate,
    Video
}

#[derive(Clone, ValueEnum, PartialEq)]
enum OutputMode {
    FullwidthOnly,
    ASCII,
    Mixed
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Output format to provide
    output_mode: OutputMode,

    /// Type of media to be processed
    media_type: DisplayType,

    /// Filepath to the target media
    path: String,

    /// Target FPS
    #[arg(short, long, default_value_t = 30)]
    fps: u32
}

fn load_video(path: &str, width: u16, height: u16, fps: u32) -> io::Result<Vec<RgbImage>> {
    let mut child = Command::new("ffmpeg")
        .args([
            "-v", "error",
            "-i", path,
            "-vf", &format!("fps={fps},scale={width}:{height}"),
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

fn get_closest_char(target: f64, args: &Args) -> (char, f64) {
    if args.output_mode == OutputMode::FullwidthOnly {
        return ('█', 1.0);
    }
    let mut best = f64::MAX;
    let mut best_ch = '\0';
    let mut best_density = f64::NAN;

    let chars = if args.output_mode == OutputMode::ASCII { ASCII_CHARS.as_slice() } else { MIXED_CHARS.as_slice() };

    for (ch, density) in chars {
        let mut diff = density - target;
        if args.output_mode == OutputMode::Mixed && diff < 0.0 {
            diff += 1e10; // In mixed mode we want the selected char's density to be >= the requirement (dimmed by colors)
        }
        diff = diff.abs();
        if diff < best {
            best = diff;
            best_ch = *ch;
            best_density = *density;
        }
    }

    assert_ne!(best_ch, '\0');
    return (best_ch, best_density);
}

fn color(mut r: u8, mut g: u8, mut b: u8, ch_density: f64, args: &Args) -> String {
    let sum_colors = r as u16 + g as u16 + b as u16;
    if sum_colors > 0 && args.output_mode != OutputMode::FullwidthOnly {
        let m = 1.0 / ch_density.powf(1.0 / 2.2); // Approximate luminance
        r = (r as f64 * m).min(255.0) as u8;
        g = (g as f64 * m).min(255.0) as u8;
        b = (b as f64 * m).min(255.0) as u8;
    }
    return format!("\x1b[38;2;{};{};{}m", r, g, b);
}

fn frame_to_str(raw_img: &RgbImage, width: u32, height: u32, map: &HashMap<i32, (char, f64)>, args: &Args) -> String {
    let img = resize(raw_img, width, height, FilterType::Triangle);

    let mut res = String::new();

    for row in 0..height {
        for col in 0..width {
            let [r, g, b] = img[(col, row)].0;
            let (mut ch, ch_density) = map[&(r.max(g).max(b) as i32)];
            // If we're animating a picture, randomly add some noise
            if args.media_type == DisplayType::Animate && rand::random::<f64>() < 0.01 {
                ch = '*';
            }
            res.push_str(&color(r, g, b, ch_density, args));
            res.push(ch);
        }
        if row != height - 1 {
            res.push('\n');
        }
    }

    return res;
}

fn main() {
    let args = Args::parse();

    #[cfg(windows)]
    if !crossterm::ansi_support::supports_ansi() {
        eprintln!("This console cannot parse ANSI colors");
        exit(1);
    }

    let (cols, lines) = terminal::size().unwrap();
    let map: HashMap<i32, (char, f64)> = (0..=(255)).map(|v| (v, get_closest_char(v as f64 / 255.0, &args))).collect();

    let start = time::Instant::now();
    let mut next_frame = time::Instant::now();
    let frame_time = time::Duration::from_millis(1000 / args.fps as u64);

    let mut stdout = io::stdout().lock();

    if args.media_type == DisplayType::Image || args.media_type == DisplayType::Animate {
        let frame = &image::open(&args.path).unwrap().to_rgb8();
        print!("\x1b[48;2;0;0;0m\x1b[2J"); // Clear + black bg
        loop {
            write!(stdout, "\x1b[?2026h\x1b[H").unwrap(); // Freeze rendering, home
            println!("{}", frame_to_str(&frame, cols as u32, lines as u32, &map, &args));
            write!(stdout, "\x1b[?2026l").unwrap(); // Unfreeze rendering

            if args.media_type != DisplayType::Animate {
                break;
            }
            next_frame += frame_time;
            thread::sleep(next_frame.saturating_duration_since(time::Instant::now()));
        }
        print!("\x1b[0m");
        exit(0);
    }

    let frames = load_video(&args.path, cols, lines, args.fps).unwrap();

    // Sound
    let mut handle = rodio::DeviceSinkBuilder::open_default_sink().unwrap();
    handle.log_on_drop(false);
    let file = File::open(&args.path).unwrap();
    let source = Decoder::try_from(file).unwrap();
    let player = Player::connect_new(handle.mixer());
    player.append(source);

    let mut displayed_frames = 0 as u64;

    print!("\x1b[48;2;0;0;0m\x1b[2J"); // Clear + black bg
    while !player.empty() {
        let time = player.get_pos().as_secs_f64();
        let frame = (time * args.fps as f64).round() as usize;
        if frame >= frames.len() {
            break;
        }
        write!(stdout, "\x1b[?2026h\x1b[H").unwrap(); // Freeze rendering, home
        write!(stdout, "{}", frame_to_str(&frames[frame], cols as u32, lines as u32, &map, &args)).unwrap();
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
