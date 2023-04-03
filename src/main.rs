use std::{
    fs::{File, metadata},
    io::{Write, BufReader, BufWriter, BufRead},
    path::Path,
};

#[derive(PartialEq, Eq)]
enum BufferState {
    NotEmpty,
    Empty,
}
impl BufferState {
    fn is_eof(&self) -> bool {
        if *self == BufferState::Empty {
            true
        }
        else {
            false
        }
    }
}

trait BufferedRead {
    fn fill_buffer(&mut self) -> BufferState;
}
impl BufferedRead for BufReader<File> {
    fn fill_buffer(&mut self) -> BufferState {
        self.consume(self.capacity());
        self.fill_buf().unwrap();
        if self.buffer().is_empty() {
            return BufferState::Empty;
        }
        BufferState::NotEmpty
    }
}

trait BufferedWrite {
    fn write_byte(&mut self, output: u8);
    fn flush_buffer(&mut self);
}
impl BufferedWrite for BufWriter<File> {
    fn write_byte(&mut self, output: u8) {
        self.write(&[output]).unwrap();
        
        if self.buffer().len() >= self.capacity() {
            self.flush().unwrap();
        }
    }
    fn flush_buffer(&mut self) {
        self.flush().unwrap();
    }
}

fn new_input_file(capacity: usize, file_name: &str) -> BufReader<File> {
    BufReader::with_capacity(
        capacity, File::open(file_name).unwrap()
    )
}

fn new_output_file(capacity: usize, file_name: &str) -> BufWriter<File> {
    BufWriter::with_capacity(
        capacity, File::create(file_name).unwrap()
    )
}


struct Match {
    pub offset: u16,
    pub len:    u16,
}
impl Match {
    fn new(offset: u16, len: u16) -> Self {
        Self {
            offset,
            len,
        }
    }
}

struct Window {
    data: Vec<u8>,
    pos:  usize,
    size: usize,
}
impl Window {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            pos:  0,
            size,
        }
    }

    fn add_byte(&mut self, byte: u8) {
        self.data[self.pos % self.size] = byte;
        self.pos += 1;
    }

    fn add_bytes(&mut self, bytes: &[u8]) {
        for byte in bytes.iter() {
            self.add_byte(*byte);
        }
    }

    fn get_byte(&self, pos: usize) -> u8 {
        self.data[pos % self.size]
    }

    fn len(&self) -> usize {
        self.data.len()
    }
}

const WINDOW_SIZE: usize = 2048;
const MAX_MATCHES: usize = 512;

struct Lz77 {
    window:   Window,
    buf_pos:  usize,
    file_in:  BufReader<File>,
    file_out: BufWriter<File>,
}
impl Lz77 {
    fn new(file_in: BufReader<File>, file_out: BufWriter<File>) -> Lz77 {
        Lz77 {
            window:   Window::new(WINDOW_SIZE),
            buf_pos:  0,
            file_in,
            file_out,
        }
    }

    fn compress(&mut self) {
        self.file_in.fill_buffer();
        let mut matches = Vec::<Match>::with_capacity(MAX_MATCHES);
        loop {
            for i in (8..self.window.len()).rev() {
                if self.window.get_byte(i) == self.file_in.buffer()[self.buf_pos] {
                    let mut m = Match::new(i as u16, 1);

                    for c in self.file_in.buffer().iter().skip(self.buf_pos + 1).take(30) {
                        if *c == self.window.get_byte((m.offset + m.len) as usize) {
                            m.len += 1;
                        } 
                        else { 
                            break; 
                        }  
                    }
                    if m.len > 1 {
                        matches.push(m);
                    }
                }
                if matches.len() == MAX_MATCHES {
                    break;
                } 
            }
            let best_match = matches.iter().reduce(|best, m| {
                if m.len > best.len { m } else { best }
            });

            if let Some(m) = best_match {
                let ptr = ((m.offset & 0x7FF) << 5) + (m.len & 31);
                self.file_out.write_byte((ptr >> 8) as u8);
                self.file_out.write_byte((ptr & 0x00FF) as u8); 

                let match_bytes = self.buf_pos..self.buf_pos + m.len as usize;
                self.window.add_bytes(&self.file_in.buffer()[match_bytes]); 

                if self.advance(m.len as usize).is_eof() { break; } 
            }
            else {
                self.file_out.write_byte(0);
                self.file_out.write_byte(self.file_in.buffer()[self.buf_pos]);
                self.window.add_byte(self.file_in.buffer()[self.buf_pos]);
                
                if self.advance(1).is_eof() { break; }
            }
            matches.clear();
        } 
        self.file_out.flush_buffer();
    }

    fn decompress(&mut self) { 
        self.file_in.fill_buffer(); 
        let mut pending = Vec::new();
        loop {
            let mut ptr = (self.file_in.buffer()[self.buf_pos] as u16) * 256;
            if self.advance(1).is_eof() { break; }
            ptr += self.file_in.buffer()[self.buf_pos] as u16;

            if (ptr >> 8) == 0 {
                self.file_out.write_byte((ptr & 0x00FF) as u8);
                self.window.add_byte(self.file_in.buffer()[self.buf_pos]);
            } 
            else { 
                let m = Match::new(ptr >> 5, ptr & 31);

                for i in 0..m.len {
                    let byte = self.window.get_byte((m.offset + i) as usize);
                    self.file_out.write_byte(byte);
                    pending.push(byte);
                }
                self.window.add_bytes(&pending);
                pending.clear();
            }
            if self.advance(1).is_eof() { break; }
        }
        self.file_out.flush_buffer();
    }

    fn advance(&mut self, len: usize) -> BufferState {
        self.buf_pos += len; 
        if self.buf_pos >= self.file_in.buffer().len() {
            self.buf_pos = 0;
            return self.file_in.fill_buffer()
        }
        BufferState::NotEmpty
    }
}

fn main() {
    let start_time = std::time::Instant::now();
    let args = std::env::args().skip(1).collect::<Vec<String>>();
    let file_in  = new_input_file(4096, &args[1]);
    let file_out = new_output_file(4096, &args[2]);

    match args[0].as_str() {
        "c" => {
            Lz77::new(file_in, file_out).compress();
            println!("Finished Compressing");
        }
        "d" => { 
            Lz77::new(file_in, file_out).decompress();
            println!("Finished Decompressing");
        }
        _ => { 
            println!("To compress: c input output.");
            println!("To decompress: d input output.");
        }
    }  
    println!("{} bytes -> {} bytes in {:.2?}", 
        metadata(Path::new(&args[1])).unwrap().len(), 
        metadata(Path::new(&args[2])).unwrap().len(), 
        start_time.elapsed()
    ); 
}  