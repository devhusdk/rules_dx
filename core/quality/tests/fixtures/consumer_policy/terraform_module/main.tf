terraform {
  required_version = ">= 1.5"
}

variable "name" {
  type    = string
  default = "example"
}

module "child" {
  source = "./modules/child"
  name   = var.name
}
