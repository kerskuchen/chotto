#![windows_subsystem = "windows"]

use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use cottontail::{
    core::{
        panic_message_split_to_message_and_location, path_exists,
        serde_derive::{Deserialize, Serialize},
    },
    image::{Color, PixelRGBA},
};

use cottontail::{
    core::{collect_files, read_file_whole},
    image::Bitmap,
    math::{Random, Shufflebag, Vec2i},
};
use rayon::iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator};

#[derive(Debug, Serialize, Deserialize)]
struct DrawParams {
    number_of_sheets_to_generate: usize,
    text_font_size: u32,
    text_color_rgb: (u8, u8, u8),
    bingo_grid_pixel_location_left_top_right_bottom: (u32, u32, u32, u32),
}

struct Input {
    background_bitmap: Bitmap,
    font: fontdue::Font,
    params: DrawParams,
}

impl Input {
    fn new() -> Input {
        let files = collect_files(".");
        if files
            .iter()
            .filter(|filepath| filepath.to_lowercase().ends_with(".png"))
            .count()
            != 1
            || files
                .iter()
                .filter(|filepath| filepath.to_lowercase().ends_with(".ttf"))
                .count()
                != 1
        {
            show_messagebox(
                "Chotto",
                "Please place exactly one PNG and one TTF file into the directory where `chotto.exe` is located and then restart Chotto",
                false,
            );
            std::process::abort();
        }

        let mut background_bitmap = Bitmap::new_empty();
        let mut font = None;
        for filepath in collect_files(".") {
            if filepath.to_lowercase().ends_with(".png") {
                background_bitmap = Bitmap::from_png_file_or_panic(&filepath);
                assert!(
                    background_bitmap.width != 0 && background_bitmap.height != 0,
                    "Image file '{}' is 0x0 pixels which is not allowed - is the file ok?",
                    filepath
                );
            }
            if filepath.to_lowercase().ends_with(".ttf") {
                let font_data = read_file_whole(&filepath)
                    .expect(&format!("Cannot read font file '{}'", filepath));
                font = Some(
                    fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default()).expect(
                        &format!("Cannot decode font file '{}' - is the file ok?", filepath),
                    ),
                );
            }
        }

        if font.is_none() {
            unreachable!();
        }

        const TOML_DOCUMENTATION_HEADER: &str =
"####################################################################################################
#
# In this file we can change various things about how Chotto should draw our Bingo-sheets by editing
# the four parameters at the bottom.
#
# The `number_of_sheets_to_generate` parameter indicates how many Bingo-sheets we want Chotto 
# to generate. The final sheets will be placed in the `output_sheets` directory once Chotto was run.
#
# The `text_font_size` and `text_color_rgb` paramters can be used to customize the final text 
# size and color. The color values are [Red, Green, Blue] in order and each range between 0-255.
# The font size is given in pixel-height. Note though that the final numbers on the grid may be 
# slightly smaller than the given font size. We can just try out some values until it looks good.
#
# The `bingo_grid_pixel_location_left_top_right_bottom` parameter defines the rectangular region
# in the image where the Bingo numbers will be drawn to. The values are [Left, Top, Right, Bottom]
# and are given in pixels.
#
# For example if we have a 100x100px image and only want numbers drawn on the bottom half of the 
# image we can write:
#
# bingo_grid_pixel_location_left_top_right_bottom = [0, 50, 100, 100]
#
####################################################################################################";
        const DRAW_PARAMETERS_FILENAME: &str = "draw_parameters.txt";
        if !path_exists(DRAW_PARAMETERS_FILENAME) {
            let params = DrawParams {
                number_of_sheets_to_generate: 10,
                text_font_size: background_bitmap.height as u32 / 20,
                text_color_rgb: (255, 128, 64),
                bingo_grid_pixel_location_left_top_right_bottom: (
                    0,
                    0,
                    background_bitmap.width as u32,
                    background_bitmap.height as u32,
                ),
            };
            let params_string = format!(
                "{}\n\n{}",
                TOML_DOCUMENTATION_HEADER,
                toml::to_string(&params).unwrap()
            );
            std::fs::write(DRAW_PARAMETERS_FILENAME, &params_string).expect(&format!(
                "Could not create file '{}'",
                DRAW_PARAMETERS_FILENAME
            ));
            show_messagebox(
                "Chotto",
                &format!(
                    "Please first fill out the '{}' in the directory where 'chotto.exe' is located and then restart Chotto",
                    DRAW_PARAMETERS_FILENAME
                ),
                false,
            );
            std::process::abort();
        }

        let params = toml::from_str(&std::fs::read_to_string(DRAW_PARAMETERS_FILENAME).expect(
            &format!("Could not read file '{}'", DRAW_PARAMETERS_FILENAME),
        ))
        .unwrap_or_else(|error| panic!("Could not read draw parameters: {}", error));

        Input {
            background_bitmap,
            font: font.unwrap(),
            params,
        }
    }
}

fn main() {
    set_panic_hook();

    let input = Input::new();
    let font = input.font;
    let background = input.background_bitmap;
    let sheet_count = input.params.number_of_sheets_to_generate;
    let top_left = Vec2i::new(
        input
            .params
            .bingo_grid_pixel_location_left_top_right_bottom
            .0 as i32,
        input
            .params
            .bingo_grid_pixel_location_left_top_right_bottom
            .1 as i32,
    );
    let bottom_right = Vec2i::new(
        input
            .params
            .bingo_grid_pixel_location_left_top_right_bottom
            .2 as i32,
        input
            .params
            .bingo_grid_pixel_location_left_top_right_bottom
            .3 as i32,
    );
    let font_size = input.params.text_font_size as f32;
    let text_color = PixelRGBA::new(
        input.params.text_color_rgb.0,
        input.params.text_color_rgb.1,
        input.params.text_color_rgb.2,
        255,
    )
    .to_color();

    let number_bitmaps_premultiplied =
        create_number_bitmaps_premultiplied(font, font_size, text_color);

    let cell_width = (bottom_right.x - top_left.x) / 5;
    let cell_height = (bottom_right.y - top_left.y) / 5;

    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();
    let seed = (since_the_epoch.as_nanos() & (std::u64::MAX as u128)) as u64;
    let randoms = Random::new_from_seed_multiple(seed, sheet_count);
    randoms
        .into_par_iter()
        .enumerate()
        .for_each(|(sheet_index, mut random)| {
            let sheet_index = sheet_index + 1;

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
                    let number_bitmap = number_bitmaps_premultiplied.get(&number).unwrap();
                    number_bitmap.blit_to_alpha_blended_premultiplied(
                        &mut background,
                        center - number_bitmap.rect().dim / 2,
                        true,
                        cottontail::image::ColorBlendMode::Normal,
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

fn create_number_bitmaps_premultiplied(
    font: fontdue::Font,
    font_size: f32,
    color: Color,
) -> HashMap<i32, Bitmap> {
    let digits_metrics_bitmaps_premultiplied: HashMap<char, (fontdue::Metrics, Bitmap)> =
        "0123456789"
            .chars()
            .map(|digit| (digit, font.rasterize(digit, font_size)))
            .map(|(digit, (metrics, image_bytes))| {
                let mut bitmap_premultiplied = Bitmap::from_greyscale_bytes_premultiplied(
                    &image_bytes,
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

    let mut number_bitmaps_premultiplied = HashMap::new();
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

        number_bitmap_premultiplied.trim_by_value(true, true, true, true, PixelRGBA::transparent());
        // number_bitmap_premultiplied
        //     .to_unpremultiplied_alpha()
        //     .write_to_png_file(&format!("target/test_numbers/{}.png", number));

        number_bitmaps_premultiplied.insert(number, number_bitmap_premultiplied);
    }
    number_bitmaps_premultiplied
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
