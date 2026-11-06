import Test.Hspec
import Hello

main :: IO ()
main = hspec $ it "is empty" $ hello `shouldBe` hello
