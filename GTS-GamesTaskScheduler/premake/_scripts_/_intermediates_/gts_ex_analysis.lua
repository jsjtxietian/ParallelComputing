project "analysis"
    kind "ConsoleApp"
    language "C++"
    targetdir "%{prj.location}/%{cfg.buildcfg}_%{cfg.architecture}"
    links { "gts" }
    includedirs {
        "../../../source/gts/include",
        "../../../source/gts/examples/6_analysis/debugging_and_instrumentation/include"
    }
    files {
        "../../../source/gts/examples/6_analysis/debugging_and_instrumentation/**.*"
    }
    filter{ "system:linux" }
        links { "pthread" }
