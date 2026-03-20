"""
Setup script for Microscope Memory Python bindings
"""

from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="microscope-memory",
    version="0.1.0",
    author="Máté Róbert (Silent)",
    author_email="your.email@example.com",
    description="Zoom-based hierarchical memory system with sub-microsecond queries",
    long_description=open("README.md").read(),
    long_description_content_type="text/markdown",
    url="https://github.com/yourusername/microscope-memory",
    rust_extensions=[
        RustExtension(
            "microscope_memory.microscope_memory",
            binding=Binding.PyO3,
            features=["python"],
        )
    ],
    packages=["microscope_memory"],
    python_requires=">=3.7",
    install_requires=[
        "numpy>=1.19.0",
    ],
    extras_require={
        "dev": [
            "pytest>=6.0",
            "black>=21.0",
            "mypy>=0.900",
        ],
        "viz": [
            "matplotlib>=3.3.0",
            "plotly>=5.0.0",
        ],
    },
    classifiers=[
        "Development Status :: 3 - Alpha",
        "Intended Audience :: Developers",
        "License :: OSI Approved :: MIT License",
        "Programming Language :: Python :: 3",
        "Programming Language :: Python :: 3.7",
        "Programming Language :: Python :: 3.8",
        "Programming Language :: Python :: 3.9",
        "Programming Language :: Python :: 3.10",
        "Programming Language :: Python :: 3.11",
        "Programming Language :: Rust",
        "Topic :: Software Development :: Libraries",
        "Topic :: Scientific/Engineering :: Artificial Intelligence",
    ],
    zip_safe=False,
)