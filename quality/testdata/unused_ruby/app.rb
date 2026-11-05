require_relative "private_lib"

class App
  def run
    PrivateLib.used
  end
end

App.new.run
