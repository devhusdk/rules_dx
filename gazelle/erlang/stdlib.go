package erlang

import (
	_ "embed"
	"strings"
)

//go:embed otp_modules.txt
var otpModulesText string

var otpModules = parseOTP(otpModulesText)

func parseOTP(text string) map[string]string {
	out := make(map[string]string)
	for _, line := range strings.Split(text, "\n") {
		fields := strings.Fields(line)
		if len(fields) == 2 && !strings.HasPrefix(fields[0], "#") {
			out[fields[0]] = fields[1]
		}
	}
	return out
}

// IsStdLib reports whether name is a module that ships with the pinned OTP.
func IsStdLib(name string) bool {
	_, ok := otpModules[name]
	return ok
}

// OTPApp returns the OTP application that ships module name.
func OTPApp(name string) (string, bool) {
	app, ok := otpModules[name]
	return app, ok
}

// IsOTPApp reports whether app is an application of the pinned OTP.
func IsOTPApp(app string) bool {
	for _, a := range otpModules {
		if a == app {
			return true
		}
	}
	return false
}

// implicitApps are started with every node, so extra_apps never lists them.
func implicitApp(app string) bool { return app == "kernel" || app == "stdlib" || app == "erts" }
