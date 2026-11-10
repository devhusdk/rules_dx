# typed: true

module Dirty
  def self.broken
    UndeclaredName.call
  end
end
