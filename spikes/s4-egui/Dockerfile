FROM rust:1.99-slim-trixie

ARG UID=1000
ARG GID=1000

# Runtime libraries that winit, glutin and wgpu open with dlopen (X11,
# xkb, GL, EGL, Vulkan), Mesa for both the host GPU (iris, anv) and
# software rendering (llvmpipe, lavapipe), and the tools the scripts use:
# Xvfb for a virtual display, xdotool and xclip for the input and
# clipboard checks, glxinfo and vulkaninfo for the versions in the
# report, and the AT-SPI stack with Python bindings for the screen
# reader check. curl and unzip fetch the fonts below.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates curl unzip \
        libx11-6 libx11-xcb1 libxcursor1 libxrandr2 libxi6 libxcb1 \
        libxkbcommon0 libxkbcommon-x11-0 \
        libgl1 libegl1 libglx-mesa0 libegl-mesa0 libgl1-mesa-dri \
        libvulkan1 mesa-vulkan-drivers \
        xvfb xauth xdotool xclip x11-utils mesa-utils vulkan-tools \
        dbus dbus-x11 at-spi2-core python3 python3-gi gir1.2-atspi-2.0 \
    && rm -rf /var/lib/apt/lists/*

# Fonts are fetched here, not committed. Both are under the SIL Open
# Font License 1.1. The versions and checksums are written next to the
# files so the report can quote them.
ARG INTER_VERSION=4.1
ARG JBMONO_VERSION=2.304
RUN mkdir -p /opt/fonts /tmp/fonts \
    && cd /tmp/fonts \
    && curl -fsSL -o inter.zip "https://github.com/rsms/inter/releases/download/v${INTER_VERSION}/Inter-${INTER_VERSION}.zip" \
    && curl -fsSL -o jbmono.zip "https://github.com/JetBrains/JetBrainsMono/releases/download/v${JBMONO_VERSION}/JetBrainsMono-${JBMONO_VERSION}.zip" \
    && sha256sum inter.zip jbmono.zip > /opt/fonts/SOURCES.txt \
    && unzip -q inter.zip -d inter && unzip -q jbmono.zip -d jbmono \
    && cp inter/extras/ttf/Inter-Regular.ttf inter/extras/ttf/Inter-Medium.ttf inter/extras/ttf/Inter-SemiBold.ttf /opt/fonts/ \
    && cp jbmono/fonts/ttf/JetBrainsMono-Regular.ttf jbmono/fonts/ttf/JetBrainsMono-Medium.ttf /opt/fonts/ \
    && cp inter/LICENSE.txt /opt/fonts/Inter-LICENSE.txt && cp jbmono/OFL.txt /opt/fonts/JetBrainsMono-OFL.txt \
    && echo "Inter ${INTER_VERSION} from github.com/rsms/inter releases" >> /opt/fonts/SOURCES.txt \
    && echo "JetBrains Mono ${JBMONO_VERSION} from github.com/JetBrains/JetBrainsMono releases" >> /opt/fonts/SOURCES.txt \
    && cd / && rm -rf /tmp/fonts

RUN (getent group "$GID" || groupadd -g "$GID" dev) \
    && (getent passwd "$UID" || useradd -m -u "$UID" -g "$GID" dev) \
    && mkdir -p /cargo /target \
    && chown "$UID:$GID" /cargo /target

ENV CARGO_HOME=/cargo \
    CARGO_TARGET_DIR=/target \
    S4_FONT_DIR=/opt/fonts
