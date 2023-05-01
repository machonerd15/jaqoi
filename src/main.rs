use std::{env, fs};
use std::process::exit;

use image::ImageFormat;

use jaqoi::Channels;

struct Config<'a> {
    input_file_name: &'a str,
    output_file_name: &'a str,
    input_image_format: ImageFormat,
    output_image_format: ImageFormat,
}

impl<'a> Config<'a> {
    fn build(arguments: &'a Vec<String>) -> Result<Config<'a>, &'static str> {
        if arguments.len() < 3 {
            return Err("Need at least 2 arguments for input filepath and output filepath")
        }
        let input_file_name = &arguments[1];
        let output_file_name = &arguments[2];
        let input_image_format = match ImageFormat::from_path(input_file_name){
            Ok(format) => format,
            Err(_) => return Err("Error getting image format from input filename")
        };
        let output_image_format = match ImageFormat::from_path(output_file_name) {
            Ok(format) => format,
            Err(_) => return Err("Error getting image format from output filename")
        };

        let config = Config{
            input_file_name,
            output_file_name,
            input_image_format,
            output_image_format,
        };

        Ok(config)

    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = match Config::build(&args) {
        Ok(config) => config,
        Err(err) => {
            println!("{}", err);
            exit(1);
        }
    };

    let img;

    if ImageFormat::Qoi == config.input_image_format {
        let (metadata, decoded_raw) = jaqoi::decode(&fs::read(config.input_file_name).unwrap().to_vec());
        img = match metadata.channels{
            Channels::RGB => {image::DynamicImage::from(image::RgbImage::from_raw(metadata.width, metadata.height, decoded_raw).unwrap())}
            Channels::RGBA => {image::DynamicImage::from(image::RgbaImage::from_raw(metadata.width, metadata.height, decoded_raw).unwrap())}
        }
    } else {
        img = image::open(config.input_file_name).unwrap();
    }

    if ImageFormat::Qoi == config.output_image_format {
        let metadata;
        let bytes;
        match img.color() {
            image::ColorType::Rgba8 => {
                metadata = jaqoi::ImgMetadata {
                    width: img.width(),
                    height: img.height(),
                    channels: Channels::RGBA,
                    colorspace: jaqoi::Colorspace::SrgbLinearAlpha,
                };
                bytes = img.as_rgba8().unwrap().as_raw();
            },
            _ => {
                metadata = jaqoi::ImgMetadata {
                    width: img.width(),
                    height: img.height(),
                    channels: Channels::RGB,
                    colorspace: jaqoi::Colorspace::SrgbLinearAlpha,
                };
                bytes = img.as_rgb8().unwrap().as_raw();
            }
        }
        // println!("Bytes: {:?}", bytes);
        let output_raw = jaqoi::encode(bytes, &metadata);
        fs::write(config.output_file_name, output_raw).expect("Error writing output file");
    } else {
        img.save(config.output_file_name).expect("Error writing output file");
    }


}