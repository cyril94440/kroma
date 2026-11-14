package expo.modules.devicehardware

import android.app.ActivityManager
import android.content.Context
import android.hardware.display.DisplayManager
import android.media.MediaCodecInfo
import android.media.MediaCodecInfo.CodecProfileLevel
import android.media.MediaCodecList
import android.media.MediaFormat
import android.os.Build
import android.view.Display
import expo.modules.kotlin.exception.Exceptions
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * The Android TV set's own CPU, memory and decoder counts, for the About screen
 * and the playback capability table. Hermes exposes no
 * `navigator.hardwareConcurrency` / `navigator.deviceMemory` and no decoder
 * inventory at all, so the numbers come from the runtime, ActivityManager and
 * MediaCodecList instead. All are cheap synchronous reads, so a plain [Function]
 * (called on the JS thread) is enough.
 */
class DeviceHardwareModule : Module() {
  private val context: Context
    get() = appContext.reactContext ?: throw Exceptions.ReactContextLost()

  override fun definition() = ModuleDefinition {
    Name("DeviceHardware")

    Function("cpuCores") {
      Runtime.getRuntime().availableProcessors()
    }

    // totalMem is a Long; JS numbers are doubles and RAM sizes sit far inside
    // the 2^53 a double holds exactly, so the width is not a concern.
    Function("memoryBytes") {
      memoryInfo().totalMem.toDouble()
    }

    Function("freeMemoryBytes") {
      memoryInfo().availMem.toDouble()
    }

    Function("decoderFrameLimits") {
      decoderFrameLimits()
    }

    Function("hdrSupport") {
      hdrSupport()
    }
  }

  private fun hdrSupport(): Map<String, List<Any>> {
    val support = mutableMapOf<String, List<Any>>(
      "dolbyVisionDecoderProfiles" to dolbyVisionDecoderProfiles()
    )
    displayHdrTypes()?.let { support["displayHdrTypes"] = it }
    return support
  }

  private fun dolbyVisionDecoderProfiles(): List<Int> {
    val profiles = sortedSetOf<Int>()
    for (codec in MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos) {
      if (codec.isEncoder) continue
      val type =
        codec.supportedTypes.firstOrNull { it.equals(MediaFormat.MIMETYPE_VIDEO_DOLBY_VISION, true) }
          ?: continue
      val levels =
        runCatching { codec.getCapabilitiesForType(type).profileLevels }.getOrNull() ?: continue
      for (level in levels) {
        DOLBY_VISION_PROFILES[level.profile]?.let { profiles.add(it) }
      }
    }
    return profiles.toList()
  }

  private fun displayHdrTypes(): List<String>? {
    val manager = context.getSystemService(Context.DISPLAY_SERVICE) as DisplayManager
    val display = manager.getDisplay(Display.DEFAULT_DISPLAY) ?: return null
    val types = supportedHdrTypes(display) ?: return null
    return types.mapNotNull { HDR_TYPES[it] }.distinct()
  }

  @Suppress("DEPRECATION")
  private fun supportedHdrTypes(display: Display): IntArray? =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      display.mode.supportedHdrTypes
    } else {
      display.hdrCapabilities?.supportedHdrTypes
    }

  // Hardware decoders only, keyed by MIME type: the software ones announce sizes
  // the CPU behind them cannot sustain (a 4K HEVC ceiling on four in-order cores
  // that reach a couple of frames a second), which is the very fallback the
  // limit exists to keep playback out of. Largest wins where a type has several.
  private fun decoderFrameLimits(): Map<String, Map<String, Int>> {
    val limits = mutableMapOf<String, Map<String, Int>>()
    for (codec in MediaCodecList(MediaCodecList.REGULAR_CODECS).codecInfos) {
      if (codec.isEncoder || !codec.usesHardware()) continue
      for (type in codec.supportedTypes) {
        val video =
          runCatching { codec.getCapabilitiesForType(type).videoCapabilities }.getOrNull()
            ?: continue
        val width = video.supportedWidths.upper
        val height = video.supportedHeights.upper
        val best = limits[type]
        if (best == null || width * height > best.getValue("width") * best.getValue("height")) {
          limits[type] = mapOf("width" to width, "height" to height)
        }
      }
    }
    return limits
  }

  private fun MediaCodecInfo.usesHardware(): Boolean =
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      isHardwareAccelerated
    } else {
      !name.startsWith("OMX.google.", true) && !name.startsWith("c2.android.", true)
    }

  private fun memoryInfo(): ActivityManager.MemoryInfo {
    val manager = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
    val info = ActivityManager.MemoryInfo()
    manager.getMemoryInfo(info)
    return info
  }
}

private val DOLBY_VISION_PROFILES =
  mapOf(
    CodecProfileLevel.DolbyVisionProfileDvavPer to 0,
    CodecProfileLevel.DolbyVisionProfileDvavPen to 1,
    CodecProfileLevel.DolbyVisionProfileDvheDer to 2,
    CodecProfileLevel.DolbyVisionProfileDvheDen to 3,
    CodecProfileLevel.DolbyVisionProfileDvheDtr to 4,
    CodecProfileLevel.DolbyVisionProfileDvheStn to 5,
    CodecProfileLevel.DolbyVisionProfileDvheDth to 6,
    CodecProfileLevel.DolbyVisionProfileDvheDtb to 7,
    CodecProfileLevel.DolbyVisionProfileDvheSt to 8,
    CodecProfileLevel.DolbyVisionProfileDvavSe to 9,
    CodecProfileLevel.DolbyVisionProfileDvav110 to 10,
  )

private val HDR_TYPES =
  mapOf(
    Display.HdrCapabilities.HDR_TYPE_DOLBY_VISION to "dolbyVision",
    Display.HdrCapabilities.HDR_TYPE_HDR10 to "hdr10",
    Display.HdrCapabilities.HDR_TYPE_HDR10_PLUS to "hdr10Plus",
    Display.HdrCapabilities.HDR_TYPE_HLG to "hlg",
  )
