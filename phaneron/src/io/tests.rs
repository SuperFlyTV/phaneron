/*
 * Phaneron media compositing software.
 * Copyright (C) 2023 SuperFlyTV AB
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use abi_stable::sabi_trait::TD_CanDowncast;
use byteorder::{ByteOrder, LittleEndian};
use phaneron_plugin::{
    traits::FromAudioF32 as FromAudioF32Trait, traits::ProcessFrameContext_TO,
    traits::ToAudioF32 as ToAudioF32Trait, AudioChannelLayout, AudioFormat,
};

use crate::{io::FromAudioF32, node_context::ProcessFrameContextImpl};

use super::ToAudioF32;

#[test]
fn from_i32_mono() {
    let to_audio_f32 = ToAudioF32::new(AudioFormat::I32, AudioChannelLayout::Mono);
    let audio = vec![i32::MAX; 1024];
    let mut audio_buf = vec![0u8; 1024 * 4];
    LittleEndian::write_i32_into(&audio, &mut audio_buf);
    let loaded = to_audio_f32.load_frame(&audio_buf.as_slice().into());
    let processed = to_audio_f32.process_frame(loaded);
    assert_eq!(processed.buffers().get(0).unwrap(), &vec![1.0f32; 1024]);
    let from_audio_f32 = FromAudioF32::new(AudioFormat::U16, AudioChannelLayout::Mono);
    let process_context = ProcessFrameContextImpl::default();
    let process_context = ProcessFrameContext_TO::from_value(process_context, TD_CanDowncast);
    let processed = from_audio_f32.process_frame(&process_context, processed);
    let frame = from_audio_f32.copy_frame(&process_context.submit().unwrap(), processed);
    assert_eq!(frame, vec![255u8; 1024 * 2]);
}
