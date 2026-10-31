# SPDX-License-Identifier: MPL-2.0
# Pinned, reproducible Intel Gen9 (SKL/KBL = Production) EU-kernel finalizer.
# IGC igc-1.0.17193.4 + compute-runtime 24.26.30049.6 (ships ocloc inside
# intel-opencl-icd). This is the README's pinned igc-1.0.17193 line, exact patch.
# Gen9-capable: CR 24.26 rates Skylake/Kaby Lake = Production (24.35+ demotes them).
FROM ubuntu:22.04
ARG IGC=1.0.17193.4
ARG CR=24.26.30049.6
ARG GMM=22.3.20
RUN apt-get update && apt-get install -y --no-install-recommends \
      wget ca-certificates ocl-icd-libopencl1 binutils && \
    rm -rf /var/lib/apt/lists/*
WORKDIR /neo
RUN set -eux; \
    igbase="https://github.com/intel/intel-graphics-compiler/releases/download/igc-${IGC}"; \
    crbase="https://github.com/intel/compute-runtime/releases/download/${CR}"; \
    wget -q "$igbase/intel-igc-core_${IGC}_amd64.deb"; \
    wget -q "$igbase/intel-igc-opencl_${IGC}_amd64.deb"; \
    wget -q "$crbase/intel-opencl-icd_${CR}_amd64.deb"; \
    wget -q "$crbase/libigdgmm12_${GMM}_amd64.deb"; \
    echo "98012c235ca1a8c953bc847eaf92d061e84a0f0c67a94b9375fcbb5f1a9ae8c0  intel-igc-core_${IGC}_amd64.deb" | sha256sum -c -; \
    echo "020d9ea61a27f0d86a2665ca56ee6f4695ddee02579b6d636995cd19f9e96ba1  intel-igc-opencl_${IGC}_amd64.deb" | sha256sum -c -; \
    dpkg -i *.deb; \
    rm -rf /neo/*.deb; \
    command -v ocloc
COPY build-kernels.sh /opt/build-kernels.sh
RUN chmod +x /opt/build-kernels.sh
WORKDIR /work
