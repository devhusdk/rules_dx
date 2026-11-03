variable "name" {
  type    = string
  default = "demo"
}

output "greeting" {
  value = "hello ${var.name}"
}
