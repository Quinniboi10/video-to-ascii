use std::{
    env,
    io::{self, Read, Write},
    process::{Command, Stdio},
    collections::HashMap,
    time,
    thread
};

use image::{
    imageops::{resize, FilterType},
    RgbImage
};

use rodio::{Decoder, Player};
use std::fs::File;

use crossterm::terminal;

const CHARS: [(char, f64); 52] = [('.', 0.05215382076250206), ('`', 0.07228915662650601), (':', 0.10430764152500412), ('-', 0.10727842878362767), (',', 0.1359960389503218), (';', 0.18814985971282389), ('"', 0.2158772074599769), ('~', 0.22313913186994552), ('^', 0.23106123122627495), ('!', 0.24508994883644167), ('_', 0.2525169169830005), ('/', 0.31292292457501236), ('r', 0.31473840567750455), ('(', 0.335368872751279), (')', 0.33784452880013205), ('<', 0.3553391648786928), ('?', 0.3606205644495792), ('v', 0.37266875722066345), ('x', 0.3952797491335204), ('c', 0.4048522858557518), ('l', 0.4249876217197557), ('i', 0.4348902459151675), ('L', 0.4438026076910381), ('s', 0.4560158441987126), ('y', 0.47186004291137146), ('t', 0.48506354183858724), ('T', 0.4878692853606206), ('o', 0.5098201023271166), ('V', 0.5365571876547285), ('2', 0.5520712988942069), ('h', 0.5637894041921109), ('a', 0.5839247400561148), ('3', 0.5888760521538208), ('e', 0.6020795510810364), ('A', 0.6095065192275954), ('q', 0.6220498432084502), ('$', 0.6364086482917973), ('K', 0.6540683281069483), ('b', 0.6586895527314738), ('9', 0.6718930516586895), ('O', 0.6803102822247895), ('0', 0.6938438686251857), ('E', 0.7090278923914838), ('g', 0.7166199042746327), ('Q', 0.7270176596798151), ('R', 0.7466578643340485), ('M', 0.7560653573196897), ('W', 0.7575507509490015), ('N', 0.7796666116520876), ('#', 0.7851130549595643), ('B', 0.8380920944050172), ('@', 1.0)];

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
        return '▉';
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
    }

    let frames = load_video(file, cols, lines).unwrap();

    let mut stdout = io::stdout().lock();

    // Sound
    let handle = rodio::DeviceSinkBuilder::open_default_sink().unwrap();
    let file = File::open(file).unwrap();
    let source = Decoder::try_from(file).unwrap();
    let player = Player::connect_new(handle.mixer());
    player.append(source);

    let start = time::Instant::now();
    let mut next_frame = time::Instant::now();
    let frame_time = time::Duration::from_millis(1000 / FPS as u64);

    print!("\x1b[2J");
    while !player.empty() {
        let time = player.get_pos().as_secs_f64();
        let frame = (time * FPS as f64).round() as usize;
        if frame >= frames.len() {
            break;
        }
        write!(stdout, "\x1b[H").unwrap();
        write!(stdout, "{}", frame_to_str(&frames[frame], cols as u32, lines as u32, &map, fullwidth_only)).unwrap();
        stdout.flush().unwrap();
        next_frame += frame_time;
        thread::sleep(next_frame.saturating_duration_since(time::Instant::now()));
    }

    let end = time::Instant::now();
    let elapsed = end.duration_since(start).as_secs();

    print!("\x1b[0m");
    println!("{} s elapsed", elapsed);
    println!("{:.2} FPS", frames.len() as f64 / elapsed as f64);
}
