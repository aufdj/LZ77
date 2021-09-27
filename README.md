# LZ77

LZ77 is a sliding window compressor with a 4096 byte lookahead buffer and 2048 byte sliding window.<br> 
<br>
Byte literals are represented as a 00 byte followed by a byte literal, and length-offset pairs are represented as an 11 bit offset and 5 bit length packed into 2 bytes.<br>
<br>
To Compress:<br>
lz77.exe c input output<br>
To Decompress: <br>
lz77.exe d input output<br>
<br>
[Benchmarks](https://sheet.zoho.com/sheet/open/1pcxk88776ef2c512445c948bee21dcbbdba5?sheet=Sheet1&range=A1)
