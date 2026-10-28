#!/usr/bin/python
DOCUMENTATION = r"""
module: hello
short_description: Says hello
description: Returns a greeting.
author: dx
options: {}
"""

from ansible.module_utils.basic import AnsibleModule


def main():
    AnsibleModule(argument_spec={}).exit_json(changed=False, msg="hello")


if __name__ == "__main__":
    main()
