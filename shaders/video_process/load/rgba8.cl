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
    __global uchar4* restrict input,
    __global float4* restrict output,
    __private unsigned int width,
    __global float* restrict gammaLut,
    __constant float4* restrict gamutMatrix
) {
    uint item = get_global_id(0);
    bool lastItemOnLine = get_local_id(0) == get_local_size(0) - 1;

    // 64 output pixels per workItem
    uint numPixels = lastItemOnLine && (0 != width % 64) ? width % 64 : 64;
    uint numLoops = numPixels;

    uint inOff = 64 * item;
    uint outOff = width * get_group_id(0) + get_local_id(0) * 64;

    // optimise loading of the 3x3 gamut matrix
    float4 gamutMat0 = gamutMatrix[0];
    float4 gamutMat1 = gamutMatrix[1];
    float4 gamutMat2 = gamutMatrix[2];
    float3 gamutMatR = (float3)(gamutMat0.s0, gamutMat0.s1, gamutMat0.s2);
    float3 gamutMatG = (float3)(gamutMat0.s3, gamutMat1.s0, gamutMat1.s1);
    float3 gamutMatB = (float3)(gamutMat1.s2, gamutMat1.s3, gamutMat2.s0);

    for (uint i=0; i<numLoops; ++i) {
        uchar4 rgba8 = input[inOff];
        float4 rgba_f = convert_float4(rgba8);

        float3 rgb;
        rgb.s0 = gammaLut[convert_ushort_sat_rte(rgba_f.s0 * 65535.0f / 255.0f)];
        rgb.s1 = gammaLut[convert_ushort_sat_rte(rgba_f.s1 * 65535.0f / 255.0f)];
        rgb.s2 = gammaLut[convert_ushort_sat_rte(rgba_f.s2 * 65535.0f / 255.0f)];

        float4 rgba;
        rgba.s0 = dot(rgb, gamutMatR);
        rgba.s1 = dot(rgb, gamutMatG);
        rgba.s2 = dot(rgb, gamutMatB);
        rgba.s3 = gammaLut[convert_ushort_sat_rte(rgba_f.s3 * 65535.0f / 255.0f)];
        output[outOff] = rgba;

        inOff++;
        outOff++;
    }
}
