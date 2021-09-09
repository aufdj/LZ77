# LZ77

LZ77 is a sliding window compressor with a 4096 byte lookahead buffer and 2048 byte sliding window.<br> 
<br>
Byte literals are not represented as length-offset pairs, and length-offset pairs are prefaced by a 01 byte, so files containing 01 bytes will not be decompressed correctly.<br>
<br>
To Compress:<br>
lz77.exe c input output<br>
To Decompress: <br>
lz77.exe d input output<br>
<br>
[Benchmarks](https://sheet.zoho.com/sheet/open/1pcxk88776ef2c512445c948bee21dcbbdba5?sheet=Sheet1&range=A1)
