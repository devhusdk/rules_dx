package common

import "strings"

var jdkPackages = []string{
	"java",
	"jdk",
	"javax.accessibility",
	"javax.annotation.processing",
	"javax.crypto",
	"javax.imageio",
	"javax.lang.model",
	"javax.management",
	"javax.naming",
	"javax.net",
	"javax.print",
	"javax.script",
	"javax.security.auth",
	"javax.security.cert",
	"javax.security.sasl",
	"javax.smartcardio",
	"javax.sound",
	"javax.sql",
	"javax.swing",
	"javax.tools",
	"javax.transaction.xa",
	"javax.xml.catalog",
	"javax.xml.crypto",
	"javax.xml.datatype",
	"javax.xml.namespace",
	"javax.xml.parsers",
	"javax.xml.stream",
	"javax.xml.transform",
	"javax.xml.validation",
	"javax.xml.xpath",
	"org.ietf.jgss",
	"org.w3c.dom",
	"org.xml.sax",
	"com.sun.management",
	"com.sun.net.httpserver",
	"com.sun.source",
	"sun.misc",
}

func InPackages(spec string, packages []string) bool {
	for _, pkg := range packages {
		if spec == pkg || strings.HasPrefix(spec, pkg+".") {
			return true
		}
	}
	return false
}

func IsJDKPlatform(spec string) bool { return InPackages(spec, jdkPackages) }
