terraform {
  required_version = ">= 1.5"
}

variable "name" {
  type = string
  default = "example"
}

output "name" {
  value = var.name
}
