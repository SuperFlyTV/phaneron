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

__kernel void read(
    __global uint4* restrict input,
    __global float4* restrict output,
    __private unsigned int width,
    __constant float4* restrict colMatrix,
    __global float* restrict gammaLut,
    __constant float4* restrict gamutMatrix
) {
    uint item = get_global_id(0);
    bool lastItemOnLine = get_local_id(0) == get_local_size(0) - 1;

    // 48 pixels per workItem = 8 input uint4s per work item
    uint numPixels = lastItemOnLine && (0 != width % 48) ? width % 48 : 48;
    uint numLoops = numPixels / 6;
    uint remain = numPixels % 6;

    uint inOff = 8 * item;
    uint outOff = width * get_group_id(0) + get_local_id(0) * 48;

    float4 colMatR = colMatrix[0];
    float4 colMatG = colMatrix[1];
    float4 colMatB = colMatrix[2];

    // optimise loading of the 3x3 gamut matrix
    float4 gamutMat0 = gamutMatrix[0];
    float4 gamutMat1 = gamutMatrix[1];
    float4 gamutMat2 = gamutMatrix[2];
    float3 gamutMatR = (float3)(gamutMat0.s0, gamutMat0.s1, gamutMat0.s2);
    float3 gamutMatG = (float3)(gamutMat0.s3, gamutMat1.s0, gamutMat1.s1);
    float3 gamutMatB = (float3)(gamutMat1.s2, gamutMat1.s3, gamutMat2.s0);

    for (uint i=0; i<numLoops; ++i) {
        uint4 w = input[inOff];

        ushort4 yuva[6];
        yuva[0] = (ushort4)((w.s0 >> 10) & 0x3ff, w.s0 & 0x3ff, (w.s0 >> 20) & 0x3ff, 1);
        yuva[1] = (ushort4)(w.s1 & 0x3ff, yuva[0].s1, yuva[0].s2, 1);
        yuva[2] = (ushort4)((w.s1 >> 20) & 0x3ff, (w.s1 >> 10) & 0x3ff, w.s2 & 0x3ff, 1);
        yuva[3] = (ushort4)((w.s2 >> 10) & 0x3ff, yuva[2].s1, yuva[2].s2, 1);
        yuva[4] = (ushort4)(w.s3 & 0x3ff, (w.s2 >> 20) & 0x3ff, (w.s3 >> 10) & 0x3ff, 1);
        yuva[5] = (ushort4)((w.s3 >> 20) & 0x3ff, yuva[4].s1, yuva[4].s2, 1);

        for (uint p=0; p<6; ++p) {
            float4 yuva_f = convert_float4(yuva[p]);
            float3 rgb;
            rgb.s0 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatR) * 65535.0f)];
            rgb.s1 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatG) * 65535.0f)];
            rgb.s2 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatB) * 65535.0f)];

            float4 rgba;
            rgba.s0 = dot(rgb, gamutMatR);
            rgba.s1 = dot(rgb, gamutMatG);
            rgba.s2 = dot(rgb, gamutMatB);
            rgba.s3 = 1.0f;
            output[outOff+p] = rgba;
        }

        inOff++;
        outOff+=6;
    }

    if (remain > 0) {
        uint4 w = input[inOff];

        ushort4 yuva[4];
        yuva[0] = (ushort4)((w.s0 >> 10) & 0x3ff, w.s0 & 0x3ff, (w.s0 >> 20) & 0x3ff, 0);
        yuva[1] = (ushort4)(w.s1 & 0x3ff, yuva[0].s1, yuva[0].s2, 0);

        if (4 == remain) {
            yuva[2] = (ushort4)((w.s1 >> 20) & 0x3ff, (w.s1 >> 10) & 0x3ff, w.s2 & 0x3ff, 0);
            yuva[3] = (ushort4)((w.s2 >> 10) & 0x3ff, yuva[2].s1, yuva[2].s2, 0);
        }

        for (uint p=0; p<remain; ++p) {
            float4 yuva_f = convert_float4(yuva[p]);
            float3 rgb;
            rgb.s0 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatR) * 65535.0f)];
            rgb.s1 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatG) * 65535.0f)];
            rgb.s2 = gammaLut[convert_ushort_sat_rte(dot(yuva_f, colMatB) * 65535.0f)];

            float4 rgba;
            rgba.s0 = dot(rgb, gamutMatR);
            rgba.s1 = dot(rgb, gamutMatG);
            rgba.s2 = dot(rgb, gamutMatB);
            rgba.s3 = 1.0f;
            output[outOff+p] = rgba;
        }
    }
}
