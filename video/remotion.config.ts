// Renders: H.264 at high quality, frames as PNG so text stays crisp.
import { Config } from '@remotion/cli/config'

Config.setVideoImageFormat('png')
Config.setCodec('h264')
Config.setCrf(14)
Config.setX264Preset('slow')
Config.setPixelFormat('yuv420p')
Config.setEntryPoint('src/index.ts')
