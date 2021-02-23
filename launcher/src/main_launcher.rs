#![windows_subsystem = "windows"]

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use cottontail::{
    core::{
        deserialize_from_json_file, panic_message_split_to_message_and_location, path_exists,
        serde_derive::{Deserialize, Serialize},
        serialize_to_json_file,
    },
    image::PixelRGBA,
};

use cottontail::{
    core::{collect_files, read_file_whole},
    image::Bitmap,
    math::{Random, Shufflebag, Vec2i},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

#[derive(Debug, Serialize, Deserialize)]
struct InputParams {
    text_font_size: f32,
    text_color: PixelRGBA,
    grid_top_left: Vec2i,
    grid_bottom_right: Vec2i,
    number_sheets_to_generate: usize,
}

struct Input {
    background_bitmap: Bitmap,
    text_font_data: Vec<u8>,
    params: InputParams,
}

impl Input {
    fn new() -> Input {
        let params = deserialize_from_json_file("input_params.txt");
        let mut background_bitmap = Bitmap::new_empty();
        let mut text_font_data = Vec::new();
        for filepath in collect_files(".") {
            if filepath.to_lowercase().ends_with(".png") {
                background_bitmap = Bitmap::from_png_file_or_panic(&filepath);
            }
            if filepath.to_lowercase().ends_with(".ttf") {
                text_font_data =
                    read_file_whole(&filepath).expect(&format!("Cannot read file '{}'", filepath));
            }
        }
        assert!(
            background_bitmap.width != 0 && background_bitmap.height != 0,
            "Please place a PNG image file into the directory where `chotto.exe` is located"
        );
        assert!(
            !text_font_data.is_empty(),
            "Please place a TTF font file into the directory where `chotto.exe` is located"
        );

        if !path_exists("input_params.txt") {
            let params = InputParams {
                text_font_size: 100.0,
                text_color: PixelRGBA::new(255, 128, 64, 255),
                grid_top_left: Vec2i::new(15, 32),
                grid_bottom_right: Vec2i::new(234, 433),
                number_sheets_to_generate: 100,
            };
            serialize_to_json_file(&params, "input_params.txt");
            panic!("Please first fill out the 'input_params.txt' in the directory where `chotto.exe` is located");
        }

        Input {
            background_bitmap,
            text_font_data,
            params,
        }
    }
}

fn main() {
    set_panic_hook();

    let input = Input::new();
    let top_left = input.params.grid_top_left;
    let bottom_right = input.params.grid_bottom_right;
    let font_size = input.params.text_font_size;
    let font_data = input.text_font_data;
    let background = input.background_bitmap;
    let color = input.params.text_color.to_color();
    let sheet_count = input.params.number_sheets_to_generate;

    let digits = "0123456789";
    let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default()).unwrap();
    let digits_metrics_bitmaps_premultiplied: HashMap<char, _> = digits
        .chars()
        .map(|digit| (digit, font.rasterize(digit, font_size)))
        .map(|(digit, (metrics, bytes))| {
            let mut bitmap_premultiplied = Bitmap::from_greyscale_bytes_premultiplied(
                &bytes,
                metrics.width as u32,
                metrics.height as u32,
            );
            for pixel in bitmap_premultiplied.data.iter_mut() {
                pixel.r = ((pixel.r as f32) * color.r) as u8;
                pixel.g = ((pixel.g as f32) * color.g) as u8;
                pixel.b = ((pixel.b as f32) * color.b) as u8;
            }
            (digit, (metrics, bitmap_premultiplied))
        })
        .collect();

    // for (digit, (_metrics, bitmap_premultiplied)) in digits_metrics_bitmaps_premultiplied.iter() {
    //     bitmap_premultiplied
    //         .to_unpremultiplied_alpha()
    //         .write_to_png_file(&format!("target/test_digits/{}.png", digit));
    // }

    let mut number_bitmaps = HashMap::new();
    for number in 1..=75 {
        let number_string = number.to_string();
        let mut layout =
            fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown);
        layout.append(
            &[&font],
            &fontdue::layout::TextStyle::new(&number_string, font_size, 0),
        );
        let glyphs = layout.glyphs().clone();

        let x_min = glyphs
            .iter()
            .fold(std::f32::MAX, |acc, glyph_pos| f32::min(acc, glyph_pos.x));
        let y_min = glyphs
            .iter()
            .fold(std::f32::MAX, |acc, glyph_pos| f32::min(acc, glyph_pos.y));
        let offset_x = if x_min < 0.0 { -x_min } else { 0.0 };
        let offset_y = if y_min < 0.0 { -y_min } else { 0.0 };
        let x_max = offset_x
            + glyphs.iter().fold(std::f32::MIN, |acc, glyph_pos| {
                f32::max(acc, glyph_pos.x + glyph_pos.width as f32)
            });
        let y_max = offset_y
            + glyphs.iter().fold(std::f32::MIN, |acc, glyph_pos| {
                f32::max(acc, glyph_pos.y + glyph_pos.height as f32)
            });

        let mut number_bitmap_premultiplied = Bitmap::new(x_max.ceil() as u32, y_max.ceil() as u32);
        for glyphpos in glyphs.iter() {
            let digit = glyphpos.key.c;
            let (_digit_metrics, digit_bitmap_premultiplied) =
                digits_metrics_bitmaps_premultiplied.get(&digit).unwrap();
            digit_bitmap_premultiplied.blit_to_alpha_blended_premultiplied(
                &mut number_bitmap_premultiplied,
                Vec2i::new(
                    (offset_x + glyphpos.x.round()) as i32,
                    (offset_y + glyphpos.y.round()) as i32,
                ),
                true,
                cottontail::image::ColorBlendMode::Normal,
            );

            // println!("{:#?}", &glyphpos);
            // println!("{:#?}", &digit_metrics);
        }

        // number_bitmap_premultiplied
        //     .to_unpremultiplied_alpha()
        //     .write_to_png_file(&format!("target/test_numbers/{}.png", number));

        number_bitmaps.insert(number, number_bitmap_premultiplied);
    }

    let cell_width = (bottom_right.x - top_left.x) / 5;
    let cell_height = (bottom_right.y - top_left.y) / 5;

    (1..=sheet_count).into_par_iter().for_each(|sheet_index| {
        let start = SystemTime::now();
        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();
        let seed = (since_the_epoch.as_nanos() & (std::u64::MAX as u128)) as u64;
        let mut random = Random::new_from_seed(seed.wrapping_add(sheet_index as u64));

        let mut background = background.clone();
        let mut col_1 = Shufflebag::new((1..=15).collect());
        let mut col_2 = Shufflebag::new((16..=30).collect());
        let mut col_3 = Shufflebag::new((31..=45).collect());
        let mut col_4 = Shufflebag::new((46..=60).collect());
        let mut col_5 = Shufflebag::new((61..=75).collect());
        for y in 0..5 {
            for x in 0..5 {
                if x == 2 && y == 2 {
                    continue;
                }
                let center = top_left
                    + Vec2i::new(
                        x * cell_width + cell_width / 2,
                        y * cell_height + cell_height / 2,
                    );

                let number = match x {
                    0 => col_1.get_next(&mut random),
                    1 => col_2.get_next(&mut random),
                    2 => col_3.get_next(&mut random),
                    3 => col_4.get_next(&mut random),
                    4 => col_5.get_next(&mut random),
                    _ => unreachable!(),
                };
                let number_bitmap = number_bitmaps.get(&number).unwrap();
                number_bitmap.blit_to_alpha_blended_premultiplied(
                    &mut background,
                    center - number_bitmap.rect().dim / 2,
                    true,
                    cottontail::image::ColorBlendMode::Multiply,
                );
            }
        }

        background
            .to_unpremultiplied_alpha()
            .write_to_png_file(&format!("output_sheets/sheet_{}.png", sheet_index));
    });

    #[cfg(not(debug_assertions))]
    show_messagebox("Chotto", "Finished creating sheets. Enjoy!", false);
}

#[cfg(windows)]
fn show_messagebox(caption: &str, message: &str, is_error: bool) {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use winapi::um::winuser::{MessageBoxW, MB_ICONERROR, MB_ICONINFORMATION, MB_OK};

    let caption_wide: Vec<u16> = std::ffi::OsStr::new(caption)
        .encode_wide()
        .chain(once(0))
        .collect();
    let message_wide: Vec<u16> = std::ffi::OsStr::new(message)
        .encode_wide()
        .chain(once(0))
        .collect();

    unsafe {
        MessageBoxW(
            null_mut(),
            message_wide.as_ptr(),
            caption_wide.as_ptr(),
            MB_OK
                | if is_error {
                    MB_ICONERROR
                } else {
                    MB_ICONINFORMATION
                },
        )
    };
}

fn set_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        let (message, location) = panic_message_split_to_message_and_location(panic_info);
        let final_message = format!("{}\n\nError occured at: {}", message, location);

        show_messagebox("Chotto Error", &final_message, true);

        // NOTE: This forces the other threads to shutdown as well
        std::process::abort();
    }));
}
