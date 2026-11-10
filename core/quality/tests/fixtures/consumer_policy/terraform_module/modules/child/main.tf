variable "name" {
  type = string
}

output "name" {
  value = upper(var.name)
}
