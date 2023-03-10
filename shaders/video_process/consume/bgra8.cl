/*
    Original work Copyright (C) 2020 Streampunk Media Ltd.
    Based on work from [Streampunk Media Ltd.](https://github.com/Streampunk/phaneron)

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
*/

__kernel void write(
    __global float4* restrict input,
    __global uchar4* restrict output,
    __private unsigned int width,
    __private unsigned int interlace,
    __global float* restrict gammaLut
) {
    bool lastItemOnLine = get_local_id(0) == get_local_size(0) - 1;

    // 64 input pixels per workItem
    uint numPixels = lastItemOnLine && (0 != width % 64) ? width % 64 : 64;
    uint numLoops = numPixels;

    uint interlaceOff = (3 == interlace) ? 1 : 0;
    uint line = get_group_id(0) * ((0 == interlace) ? 1 : 2) + interlaceOff;
    uint inOff = width * line + get_local_id(0) * 64;
    uint outOff = width * line + get_local_id(0) * 64;

    for (uint i=0; i<numLoops; ++i) {
        uchar4 bgra;

        float4 rgba_l = input[inOff];
        float3 rgb_f;
        rgb_f.s0 = gammaLut[convert_ushort_sat_rte(rgba_l.s0 * 65535.0f)];
        rgb_f.s1 = gammaLut[convert_ushort_sat_rte(rgba_l.s1 * 65535.0f)];
        rgb_f.s2 = gammaLut[convert_ushort_sat_rte(rgba_l.s2 * 65535.0f)];

        bgra.s0 = convert_uchar_sat_rte(rgb_f.s2 * 255.0f);
        bgra.s1 = convert_uchar_sat_rte(rgb_f.s1 * 255.0f);
        bgra.s2 = convert_uchar_sat_rte(rgb_f.s0 * 255.0f);
        bgra.s3 = 255;
        output[outOff] = bgra;

        inOff++;
        outOff++;
    }
}
