local settings = import 'lib/settings.libsonnet';

{
  name: 'app',
  retries: settings.retries,
  timeout: settings.timeout,
}
