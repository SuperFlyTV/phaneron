/*
    Phaneron media compositing software.
    Original work Copyright (C) 2020 Streampunk Media Ltd.
    Based on work from [Streampunk Media Ltd.](https://github.com/Streampunk/phaneron)
    Further work Copyright (C) 2023 SuperFlyTV AB.

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
    __global uchar8* restrict outputY,
    __global uchar4* restrict outputU,
    __global uchar4* restrict outputV,
    __private unsigned int width,
    __private unsigned int interlace,
    __constant float4* restrict colMatrix,
    __global float* restrict gammaLut
) {
    bool lastItemOnLine = get_local_id(0) == get_local_size(0) - 1;

    // 64 input pixels per workItem = 8 input luma uchar8s per work item, 8 each u & v uchar4s per work item
    uint numPixels = lastItemOnLine && (0 != width % 64) ? width % 64 : 64;
    uint numLoops = numPixels / 8;
    uint remain = numPixels % 8;

    uint interlaceOff = (3 == interlace) ? 1 : 0;
    uint line = get_group_id(0) * ((0 == interlace) ? 1 : 2) + interlaceOff;
    uint inOff = width * line + get_local_id(0) * 64;

    uint pitchReads = (width + 7) / 8;
    uint outOff = pitchReads * line + get_local_id(0) * 8;

    float4 matY = colMatrix[0];
    float4 matU = colMatrix[1];
    float4 matV = colMatrix[2];

    for (uint i=0; i<numLoops; ++i) {
        uchar3 yuv[8];

        for (uint p=0; p<8; ++p) {
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

        uchar8 y = (uchar8)(yuv[0].s0, yuv[1].s0, yuv[2].s0, yuv[3].s0, yuv[4].s0, yuv[5].s0, yuv[6].s0, yuv[7].s0);
        uchar4 u = (uchar4)(yuv[0].s1, yuv[2].s1, yuv[4].s1, yuv[6].s1);
        uchar4 v = (uchar4)(yuv[0].s2, yuv[2].s2, yuv[4].s2, yuv[6].s2);
        outputY[outOff] = y;
        outputU[outOff] = u;
        outputV[outOff] = v;

        inOff+=8;
        outOff++;
    }

    if (remain > 0) {
        uchar8 y = (uchar8)(16, 16, 16, 16, 16, 16, 16, 16);
        uchar4 u = (uchar4)(128, 128, 128, 128);
        uchar4 v = (uchar4)(128, 128, 128, 128);

        uchar3 yuv[6];
        for (uint p=0; p<remain; ++p) {
            float4 rgba_l = input[inOff+p];
            float4 rgba;
            rgba.s0 = gammaLut[convert_ushort_sat_rte(rgba_l.s0 * 65535.0f)];
            rgba.s1 = gammaLut[convert_ushort_sat_rte(rgba_l.s1 * 65535.0f)];
            rgba.s2 = gammaLut[convert_ushort_sat_rte(rgba_l.s2 * 65535.0f)];
            rgba.s3 = 1.0;

            yuv[p].s0 = convert_ushort_sat_rte(round(dot(rgba, matY)));
            yuv[p].s1 = convert_ushort_sat_rte(round(dot(rgba, matU)));
            yuv[p].s2 = convert_ushort_sat_rte(round(dot(rgba, matV)));
        }

        y.s0 = yuv[0].s0;
        y.s1 = yuv[1].s0;
        u.s0 = yuv[0].s1;
        v.s0 = yuv[0].s2;
        if (remain > 2) {
            y.s2 = yuv[2].s0;
            y.s3 = yuv[3].s0;
            u.s1 = yuv[2].s1;
            v.s1 = yuv[2].s2;
            if (remain > 4) {
                y.s4 = yuv[4].s0;
                y.s5 = yuv[5].s0;
                u.s1 = yuv[4].s1;
                v.s1 = yuv[4].s2;
            }
        }

        outputY[outOff] = y;
        outputU[outOff] = u;
        outputV[outOff] = v;
    }
}
