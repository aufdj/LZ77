use std::fs::File;
use std::fs::metadata;
use std::io::{Write, BufReader, BufWriter, BufRead};
use std::env;
use std::time::Instant;
use std::path::Path;

// Convenience function for buffered IO ---------------------------
fn write(buf_out: &mut BufWriter<File>, output: u8) {
    buf_out.write(&[output]).unwrap();
    if buf_out.buffer().len() >= buf_out.capacity() { 
        buf_out.flush().unwrap(); 
    }
}
// -----------------------------------------------------------------

const BUFFER_SIZE: usize = 4096;
const WINDOW_SIZE: usize = 2048;
const MATCH_CODE: u8 = 1;
const MAX_MATCHES: usize = 512;

fn slide(coding_position: &mut usize, match_length: u8, file_in: &mut BufReader<File>, window: &mut [u8; WINDOW_SIZE]) -> usize {
    // slide window forward match_length bytes
    for i in 0..match_length {
        window.rotate_left(1);
        window[WINDOW_SIZE - 1] = file_in.buffer()[*coding_position + (i as usize)];
    }
    // move coding_position forward match_length bytes and check for end of buffer
    *coding_position += match_length as usize; 
    if *coding_position >= file_in.buffer().len() {
        *coding_position = 0;
        file_in.consume(file_in.capacity()); 
        file_in.fill_buf().unwrap();
        
        if file_in.buffer().is_empty() {
            return 1;
        }
    }
    0
}
fn inc_coding_pos(coding_position: &mut usize, file_in: &mut BufReader<File>) -> usize {
    *coding_position += 1 as usize; 
    if *coding_position >= file_in.buffer().len() {
    *coding_position = 0;
    file_in.consume(file_in.capacity()); 
    file_in.fill_buf().unwrap();
        
        if file_in.buffer().is_empty() {
            return 1;
        }
    }
    0
}

fn main() {
    let start_time = Instant::now();
    let args: Vec<String> = env::args().collect();
    let mut file_in  = BufReader::with_capacity(BUFFER_SIZE, File::open(&args[2]).unwrap());
    let mut file_out = BufWriter::with_capacity(BUFFER_SIZE, File::create(&args[3]).unwrap());
    file_in.fill_buf().unwrap();

    match (&args[1]).as_str() {
        "c" => {
            let mut window = [0u8; WINDOW_SIZE]; 
            let mut coding_position: usize = 0;

            let mut match_offsets = [0u16; MAX_MATCHES];
            let mut match_offset: u16 = 0; 
            let mut match_length: u8 = 1; 
            let mut longest_match_length: u8 = 1;
            
            let mut match_found = false;
            let mut end_of_window = false;

            let file_in_size = metadata(Path::new(&args[2])).unwrap().len();

            'outer_c: loop {
                // Find up to 512 matches 
                let mut num_matches: usize = 0;
                for i in (0..window.len()).rev() {  
                    if window[i] != 0 {
                        if file_in.buffer()[coding_position] == window[i] { 
                            match_offsets.rotate_right(1);
                            match_offsets[0] = i as u16;
                            num_matches += 1;
                            if num_matches >= MAX_MATCHES - 1 {
                                break;
                            }
                            match_found = true;
                        }
                    }        
                }
    
                // Find the length for each match and pick the longest one
                for i in 0..match_offsets.len() {
                    if match_offsets[i] != 0 && match_offsets[i] != (WINDOW_SIZE - 1) as u16 {
                        for j in (coding_position + 1)..(file_in.buffer().len()) { 
                            if match_offsets[i] + (match_length as u16) >= (WINDOW_SIZE - 1) as u16 {
                                end_of_window = true;
                            }
                            if file_in.buffer()[j] == window[(match_offsets[i] as usize) 
                                + (match_length as usize)] && match_length < 31 {
                                match_length += 1;    
                            } else {
                                break;
                            }
                            if end_of_window == true {
                                break;
                            }     
                        }
                        if match_length > longest_match_length {
                            longest_match_length = match_length;
                            match_offset = match_offsets[i];
                        }
                        match_length = 1;
                    }
                }
                match_length = longest_match_length;

                if match_offset == 0 || match_offset == (WINDOW_SIZE - 1) as u16 {
                    match_found == false;
                    match_length = 1;
                }

                // Write byte literal or pointer and slide coding_position/window forward
                if match_found == false {
                    write(&mut file_out, file_in.buffer()[coding_position]);
                    if slide(&mut coding_position, match_length, &mut file_in, &mut window) == 1 { 
                        break 'outer_c; 
                    }
                } else {
                    if match_length > 3 {
                        let pointer = (match_offset & 0x7FF) + (((match_length & 31) as u16) << 11);
                        write(&mut file_out, MATCH_CODE); 
                        write(&mut file_out, (pointer & 0x00FF) as u8); 
                        write(&mut file_out, (pointer >> 8) as u8);     
                        if slide(&mut coding_position, match_length, &mut file_in, &mut window) == 1 { 
                            break 'outer_c; 
                        }
                    } else {
                        for i in 0..match_length {
                            write(&mut file_out, file_in.buffer()[coding_position + (i as usize)]);
                        }
                        if slide(&mut coding_position, match_length, &mut file_in, &mut window) == 1 { 
                            break 'outer_c; 
                        }
                    }
                }

                match_length = 1;                   // reset variables
                longest_match_length = 1;           //
                match_offset = 0;                   //
                match_found = false;                //
                end_of_window = false;              // 
                for i in 0..match_offsets.len() {   //
                    match_offsets[i] = 0;           //
                }                                   //
            } 

            file_out.flush().unwrap();
            let file_out_size = metadata(Path::new(&args[3])).unwrap().len();
            println!("Finished Compressing");
            println!("{} bytes -> {} bytes in {:.2?}", file_in_size, file_out_size, start_time.elapsed());
        }
        "d" => { 
            let mut window = [0u8; WINDOW_SIZE];
            let mut coding_position: usize = 0;  
            let mut match_length: u8 = 1; 
            let mut bytes_to_write = [0u8; 32];

            let file_in_size = metadata(Path::new(&args[2])).unwrap().len();

            'outer_d: loop {
                // If the current byte is the match code, read the pointer, output bytes, and slide forward
                if file_in.buffer()[coding_position] == MATCH_CODE {
                    if inc_coding_pos(&mut coding_position, &mut file_in) == 1 { break 'outer_d; }
                    let mut pointer = file_in.buffer()[coding_position] as u16;
                    if inc_coding_pos(&mut coding_position, &mut file_in) == 1 { break 'outer_d; }
                    pointer += (file_in.buffer()[coding_position] as u16) * 256;

                    let match_offset = pointer & 0x7FF;
                    match_length = ((pointer >> 11) & 31) as u8;

                    for i in 0..match_length {
                        write(&mut file_out, window[(match_offset + i as u16) as usize]);
                        bytes_to_write[i as usize] = window[(match_offset + i as u16) as usize];
                    }

                    for i in 0..match_length {
                        window.rotate_left(1);
                        window[WINDOW_SIZE - 1] = bytes_to_write[i as usize];
                    }

                    if inc_coding_pos(&mut coding_position, &mut file_in) == 1 { break 'outer_d; }
                } else {
                    write(&mut file_out, file_in.buffer()[coding_position]);
                    if slide(&mut coding_position, match_length, &mut file_in, &mut window) == 1 { 
                        break 'outer_d; 
                    }
                }
                match_length = 1;                   // reset variables
                for i in 0..bytes_to_write.len() {  //
                    bytes_to_write[i] = 0;          //
                }                                   //
            }
            file_out.flush().unwrap();
            let file_out_size = metadata(Path::new(&args[3])).unwrap().len();
            println!("Finished Decompressing");
            println!("{} bytes -> {} bytes in {:.2?}", file_in_size, file_out_size, start_time.elapsed());
        }
        _ => { 
            println!("To compress: c input output.");
            println!("To decompress: c input output.");
            std::process::exit(1);
        }
    }   
}    

