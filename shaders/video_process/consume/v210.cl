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
    __global uint4* restrict output,
    __private unsigned int width,
    __private unsigned int interlace,
    __constant float4* restrict colMatrix,
    __global float* restrict gammaLut
) {
    bool lastItemOnLine = get_local_id(0) == get_local_size(0) - 1;

    // 48 pixels per workItem = 8 output uint4s per work item
    uint numPixels = lastItemOnLine && (0 != width % 48) ? width % 48 : 48;
    uint numLoops = numPixels / 6;
    uint remain = numPixels % 6;

    uint interlaceOff = (3 == interlace) ? 1 : 0;
    uint line = get_group_id(0) * ((0 == interlace) ? 1 : 2) + interlaceOff;
    uint inOff = width * line + get_local_id(0) * 48;
    uint outOff = width * line / 6 + get_local_id(0) * 8;

    if (48 != numPixels) {
        // clear the output buffer for the last item, partially overwritten below
        uint clearOff = outOff;
        for (uint i=0; i<8; ++i) {
            output[clearOff++] = (uint4)(0, 0, 0, 0);
        }
    }

    float4 matY = colMatrix[0];
    float4 matU = colMatrix[1];
    float4 matV = colMatrix[2];

    for (uint i=0; i<numLoops; ++i) {
        ushort3 yuv[6];

        for (uint p=0; p<6; ++p) {
            float4 rgba_l = input[inOff+p];
            float4 rgba;
            rgba.s0 = gammaLut[convert_ushort_sat_rte(rgba_l.s0 * 65535.0f)];
            rgba.s1 = gammaLut[convert_ushort_sat_rte(rgba_l.s1 * 65535.0f)];
            rgba.s2 = gammaLut[convert_ushort_sat_rte(rgba_l.s2 * 65535.0f)];
            rgba.s3 = 1.0f;

            yuv[p].s0 = convert_ushort_sat_rte(dot(rgba, matY));
            yuv[p].s1 = convert_ushort_sat_rte(dot(rgba, matU));
            yuv[p].s2 = convert_ushort_sat_rte(dot(rgba, matV));
        }

        uint4 w;
        w.s0 = yuv[0].s2 << 20 | yuv[0].s0 << 10 | yuv[0].s1;
        w.s1 = yuv[2].s0 << 20 | yuv[2].s1 << 10 | yuv[1].s0;
        w.s2 = yuv[4].s1 << 20 | yuv[3].s0 << 10 | yuv[2].s2;
        w.s3 = yuv[5].s0 << 20 | yuv[4].s2 << 10 | yuv[4].s0;
        output[outOff] = w;

        inOff+=6;
        outOff++;
    }

    if (remain > 0) {
        uint4 w = (uint4)(0, 0, 0, 0);

        ushort3 yuv[4];
        for (uint p=0; p<remain; ++p) {
            float4 rgba_l = input[inOff+p];
            float4 rgba;
            rgba.s0 = gammaLut[convert_ushort_sat_rtz(rgba_l.s0 * 65535.0f)];
            rgba.s1 = gammaLut[convert_ushort_sat_rtz(rgba_l.s1 * 65535.0f)];
            rgba.s2 = gammaLut[convert_ushort_sat_rtz(rgba_l.s2 * 65535.0f)];
            rgba.s3 = 1.0;

            yuv[p].s0 = convert_ushort_sat(round(dot(rgba, matY)));
            yuv[p].s1 = convert_ushort_sat(round(dot(rgba, matU)));
            yuv[p].s2 = convert_ushort_sat(round(dot(rgba, matV)));
        }

        w.s0 = yuv[0].s2 << 20 | yuv[0].s0 << 10 | yuv[0].s1;
        if (2 == remain) {
            w.s1 = yuv[1].s0;
        } else if (4 == remain) {
            w.s1 = yuv[2].s0 << 20 | yuv[2].s1 << 10 | yuv[1].s0;
            w.s2 = yuv[3].s0 << 10 | yuv[2].s2;
        }
        output[outOff] = w;
    }
}
